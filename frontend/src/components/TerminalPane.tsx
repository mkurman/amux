import { useEffect, useRef, useCallback, useMemo, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { CanvasAddon } from "@xterm/addon-canvas";
import { SearchAddon } from "@xterm/addon-search";
import { SerializeAddon } from "@xterm/addon-serialize";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { TerminalContextMenu } from "./terminal-pane/TerminalContextMenu";
import { buildTerminalContextMenuItems } from "./terminal-pane/menuItems";
import { useTerminalClipboard } from "./terminal-pane/useTerminalClipboard";
import { TerminalPaneHeader } from "./terminal-pane/TerminalPaneHeader";
import { useTerminalTranscript } from "./terminal-pane/useTerminalTranscript";
import {
  countSearchMatches,
  decodeBase64ToBytes,
  decodeBase64ToText,
  encodeTextToBase64,
  findPaneLocationValue,
  getRenderedTerminalText,
  getSearchableBufferText,
  quotePathForShell,
  stripAnsi,
  wrapBracketedPaste,
} from "./terminal-pane/utils";
import { useWorkspaceStore } from "../lib/workspaceStore";
import { useSettingsStore } from "../lib/settingsStore";
import { useCommandLogStore } from "../lib/commandLogStore";
import { useTranscriptStore } from "../lib/transcriptStore";
import { assessCommandRisk, useAgentMissionStore } from "../lib/agentMissionStore";
import { registerTerminalController, type TerminalSendOptions } from "../lib/terminalRegistry";
import { allLeafIds } from "../lib/bspTree";
import { getEffectiveTheme } from "../lib/themes";
import { SharedCursor } from "./SharedCursor";
import "@xterm/xterm/css/xterm.css";

interface TerminalPaneProps {
  paneId: string;
  sessionId?: string;
}

type DaemonState = "checking" | "reachable" | "unavailable";

type ContextMenuState = {
  visible: boolean;
  x: number;
  y: number;
};

/**
 * Renders a single xterm.js instance connected to a daemon session.
 *
 * On mount:
 * 1. Creates an xterm.js Terminal.
 * 2. Requests a session from the daemon (via Tauri invoke).
 * 3. Pipes input to the daemon and renders output.
 * 4. Uses xterm-addon-fit to auto-resize.
 */
export function TerminalPane({ paneId, sessionId }: TerminalPaneProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const searchAddonRef = useRef<SearchAddon | null>(null);
  const serializeAddonRef = useRef<SerializeAddon | null>(null);
  const requestedSessionIdRef = useRef(sessionId);
  const sessionReadyRef = useRef(false);
  const platformRef = useRef("linux");
  const commandBufferRef = useRef("");
  const inputSequenceStateRef = useRef({
    inEscape: false,
    inCsi: false,
    inOsc: false,
    oscEscape: false,
  });
  const lastShellCommandRef = useRef<{ command: string; timestamp: number } | null>(null);
  const commandPathRef = useRef<string>("human-typed");
  const approvalCommandByIdRef = useRef<Record<string, string>>({});
  const setActivePaneId = useWorkspaceStore((s) => s.setActivePaneId);
  const activePaneId = useWorkspaceStore((s) => s.activePaneId);
  const splitActive = useWorkspaceStore((s) => s.splitActive);
  const closePane = useWorkspaceStore((s) => s.closePane);
  const toggleZoom = useWorkspaceStore((s) => s.toggleZoom);
  const toggleSearch = useWorkspaceStore((s) => s.toggleSearch);
  const setPaneSessionId = useWorkspaceStore((s) => s.setPaneSessionId);
  const setPaneName = useWorkspaceStore((s) => s.setPaneName);
  const paneName = useWorkspaceStore(
    useCallback(
      (state) => {
        for (const workspace of state.workspaces) {
          for (const surface of workspace.surfaces) {
            if (allLeafIds(surface.layout).includes(paneId)) {
              return surface.paneNames[paneId] ?? paneId;
            }
          }
        }

        return paneId;
      },
      [paneId],
    ),
  );
  const paneWorkspaceId = useWorkspaceStore(
    useCallback(
      (state) => findPaneLocationValue(state.workspaces, paneId, (location) => location.workspaceId),
      [paneId]
    )
  );
  const paneSurfaceId = useWorkspaceStore(
    useCallback(
      (state) => findPaneLocationValue(state.workspaces, paneId, (location) => location.surfaceId),
      [paneId]
    )
  );
  const paneWorkspaceCwd = useWorkspaceStore(
    useCallback(
      (state) => findPaneLocationValue(state.workspaces, paneId, (location) => location.cwd),
      [paneId]
    )
  );
  const settings = useSettingsStore((s) => s.settings);
  const addCommandLogEntry = useCommandLogStore((s) => s.addEntry);
  const completeLatestPendingEntry = useCommandLogStore((s) => s.completeLatestPendingEntry);
  const addTranscript = useTranscriptStore((s) => s.addTranscript);
  const upsertLiveTranscript = useTranscriptStore((s) => s.upsertLiveTranscript);
  const recordSessionReady = useAgentMissionStore((s) => s.recordSessionReady);
  const recordCommandStarted = useAgentMissionStore((s) => s.recordCommandStarted);
  const recordCommandFinished = useAgentMissionStore((s) => s.recordCommandFinished);
  const recordSessionExited = useAgentMissionStore((s) => s.recordSessionExited);
  const recordError = useAgentMissionStore((s) => s.recordError);
  const recordCognitiveOutput = useAgentMissionStore((s) => s.recordCognitiveOutput);
  const requestApproval = useAgentMissionStore((s) => s.requestApproval);
  const markApprovalHandled = useAgentMissionStore((s) => s.markApprovalHandled);
  const isCommandAllowed = useAgentMissionStore((s) => s.isCommandAllowed);
  const upsertDaemonApproval = useAgentMissionStore((s) => s.upsertDaemonApproval);
  const setSharedCursorMode = useAgentMissionStore((s) => s.setSharedCursorMode);
  const setHistoryResults = useAgentMissionStore((s) => s.setHistoryResults);
  const setSymbolHits = useAgentMissionStore((s) => s.setSymbolHits);
  const setSnapshots = useAgentMissionStore((s) => s.setSnapshots);
  const approvals = useAgentMissionStore((s) => s.approvals);
  const bracketedPasteRef = useRef(settings.bracketedPaste);
  const autoCopyOnSelectRef = useRef(settings.autoCopyOnSelect);
  const pendingApprovalIdRef = useRef<string | null>(null);
  const [daemonState, setDaemonState] = useState<DaemonState>("checking");
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
  });
  const [paneNameDraft, setPaneNameDraft] = useState(paneName);
  const sharedCursorMode = useAgentMissionStore((s) => s.sharedCursorMode);
  const resolvedApprovals = useMemo(
    () => approvals.filter((entry) => entry.paneId === paneId && entry.status !== "pending" && entry.handledAt === null),
    [approvals, paneId],
  );

  useEffect(() => {
    requestedSessionIdRef.current = sessionId;
  }, [paneId, sessionId]);

  useEffect(() => {
    setPaneNameDraft(paneName);
  }, [paneName]);

  useEffect(() => {
    bracketedPasteRef.current = settings.bracketedPaste;
    autoCopyOnSelectRef.current = settings.autoCopyOnSelect;
  }, [settings.bracketedPaste, settings.autoCopyOnSelect]);

  const hideContextMenu = useCallback(() => {
    setContextMenu((current) =>
      current.visible ? { ...current, visible: false } : current
    );
  }, []);

  const commitCommandBuffer = useCallback(() => {
    const command = commandBufferRef.current.replace(/\s+/g, " ").trim();
    commandBufferRef.current = "";

    if (!command) return;

    const lastShellCommand = lastShellCommandRef.current;
    if (
      lastShellCommand
      && lastShellCommand.command === command
      && Date.now() - lastShellCommand.timestamp <= 1500
    ) {
      return;
    }

    addCommandLogEntry({
      command,
      path: commandPathRef.current,
      cwd: paneWorkspaceCwd ?? null,
      workspaceId: paneWorkspaceId ?? null,
      surfaceId: paneSurfaceId ?? null,
      paneId,
    });
    commandPathRef.current = "human-typed";
  }, [addCommandLogEntry, paneId, paneSurfaceId, paneWorkspaceCwd, paneWorkspaceId]);

  const trackInput = useCallback((text: string) => {
    const sequenceState = inputSequenceStateRef.current;

    for (const char of text) {
      const code = char.charCodeAt(0);

      if (sequenceState.inOsc) {
        if (sequenceState.oscEscape) {
          sequenceState.oscEscape = false;
          if (char === "\\") {
            sequenceState.inOsc = false;
          }
          continue;
        }

        if (char === "\u0007") {
          sequenceState.inOsc = false;
          continue;
        }

        if (char === "\u001b") {
          sequenceState.oscEscape = true;
        }
        continue;
      }

      if (sequenceState.inCsi) {
        if (code >= 0x40 && code <= 0x7e) {
          sequenceState.inCsi = false;
        }
        continue;
      }

      if (sequenceState.inEscape) {
        sequenceState.inEscape = false;
        if (char === "[") {
          sequenceState.inCsi = true;
        } else if (char === "]") {
          sequenceState.inOsc = true;
          sequenceState.oscEscape = false;
        }
        continue;
      }

      if (char === "\u001b") {
        sequenceState.inEscape = true;
        continue;
      }

      if (char === "\r" || char === "\n") {
        commitCommandBuffer();
        continue;
      }

      if (char === "\u007f" || char === "\b") {
        commandBufferRef.current = commandBufferRef.current.slice(0, -1);
        continue;
      }

      if (char === "\u0015") {
        commandBufferRef.current = "";
        continue;
      }

      if (code >= 0x20 || char === "\t") {
        commandBufferRef.current += char;
      }
    }
  }, [commitCommandBuffer]);

  const sendTextInput = useCallback(async (
    text: string,
    options?: TerminalSendOptions,
  ) => {
    if (!text) return false;

    const amux = (window as any).tamux ?? (window as any).amux;
    if (!sessionReadyRef.current) return false;

    if (options?.execute && options?.managed !== false) {
      if (!amux?.executeManagedCommand) return false;
      const managedPath = options?.source === "agent"
        ? "assistant-managed"
        : options?.source === "gateway"
          ? "gateway-managed"
          : options?.source === "replay"
            ? "replay-managed"
            : "human-managed";
      addCommandLogEntry({
        command: text.trim(),
        path: managedPath,
        cwd: paneWorkspaceCwd ?? null,
        workspaceId: paneWorkspaceId ?? null,
        surfaceId: paneSurfaceId ?? null,
        paneId,
      });
      await amux.executeManagedCommand(paneId, {
        command: text,
        rationale: options?.rationale ?? "Managed execution requested from the terminal UI",
        allowNetwork: options?.allowNetwork ?? settings.sandboxNetworkEnabled,
        sandboxEnabled: options?.sandboxEnabled ?? settings.sandboxEnabled,
        securityLevel: settings.securityLevel,
        cwd: paneWorkspaceCwd ?? undefined,
        languageHint: options?.languageHint ?? "shell",
        source: options?.source ?? "agent",
      });
      return true;
    }

    if (!amux?.sendTerminalInput) return false;

    let payload = options?.execute ? `${text}\r` : text;
    if (options?.trackHistory !== false) {
      commandPathRef.current = options?.bracketed ? "human-paste" : "human-typed";
      trackInput(payload);
    }

    payload = options?.bracketed ? wrapBracketedPaste(payload, bracketedPasteRef.current) : payload;
    await amux.sendTerminalInput(paneId, encodeTextToBase64(payload));
    return true;
  }, [paneId, trackInput]);

  const { writeClipboardText, copySelection, pasteClipboard } = useTerminalClipboard({
    termRef,
    sendTextInput,
  });
  const { captureTranscript, captureRollingTranscript } = useTerminalTranscript({
    termRef,
    serializeAddonRef,
    addTranscript,
    upsertLiveTranscript,
    paneId,
    paneWorkspaceId,
    paneSurfaceId,
    paneWorkspaceCwd,
  });

  const handleClosePane = useCallback(() => {
    closePane(paneId);
  }, [closePane, paneId]);

  const sendResize = useCallback(() => {
    const term = termRef.current;
    const amux = (window as any).tamux ?? (window as any).amux;
    if (!term || !amux?.resizeTerminalSession) return;
    void amux.resizeTerminalSession(paneId, term.cols, term.rows);
  }, [paneId]);

  const handleFocus = useCallback(() => {
    setActivePaneId(paneId);
    termRef.current?.focus();
  }, [paneId, setActivePaneId]);

  useEffect(() => {
    if (resolvedApprovals.length === 0) return;

    const amux = (window as any).tamux ?? (window as any).amux;
    const pendingId = pendingApprovalIdRef.current;
    if (!pendingId) return;
    const approval = resolvedApprovals.find((entry) => entry.id === pendingId) ?? resolvedApprovals[0];
    if (!approval || !amux?.sendTerminalInput) return;

    if (approval.status === "approved-once" || approval.status === "approved-session") {
      void amux.sendTerminalInput(paneId, encodeTextToBase64("\r"));
    } else if (approval.status === "denied") {
      commandBufferRef.current = "";
      void amux.sendTerminalInput(paneId, encodeTextToBase64("\u0003"));
    }

    pendingApprovalIdRef.current = null;
    markApprovalHandled(approval.id);
  }, [markApprovalHandled, paneId, resolvedApprovals]);

  useEffect(() => {
    let disposed = false;
    const amux = (window as any).tamux ?? (window as any).amux;

    void amux?.getPlatform?.().then((value: string) => {
      if (!disposed && typeof value === "string" && value) {
        platformRef.current = value;
      }
    });

    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    if (!containerRef.current) return;

    const themeColors = getEffectiveTheme(
      settings.themeName,
      settings.useCustomTerminalColors,
      settings.customTerminalBackground,
      settings.customTerminalForeground,
      settings.customTerminalCursor,
      settings.customTerminalSelection,
    );

    const term = new Terminal({
      allowProposedApi: true,
      fontFamily: settings.fontFamily || '"Cascadia Code", "JetBrains Mono", "Fira Code", "Consolas", monospace',
      fontSize: settings.fontSize,
      lineHeight: settings.lineHeight,
      cursorBlink: settings.cursorBlink,
      cursorStyle: settings.cursorStyle,
      scrollback: settings.scrollbackLines,
      theme: {
        background: themeColors.background,
        foreground: themeColors.foreground,
        cursor: themeColors.cursor,
        selectionBackground: themeColors.selectionBg,
        black: themeColors.black,
        red: themeColors.red,
        green: themeColors.green,
        yellow: themeColors.yellow,
        blue: themeColors.blue,
        magenta: themeColors.magenta,
        cyan: themeColors.cyan,
        white: themeColors.white,
        brightBlack: themeColors.brightBlack,
        brightRed: themeColors.brightRed,
        brightGreen: themeColors.brightGreen,
        brightYellow: themeColors.brightYellow,
        brightBlue: themeColors.brightBlue,
        brightMagenta: themeColors.brightMagenta,
        brightCyan: themeColors.brightCyan,
        brightWhite: themeColors.brightWhite,
      },
    });

    const fitAddon = new FitAddon();
    const searchAddon = new SearchAddon();
    const serializeAddon = new SerializeAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(searchAddon);
    term.loadAddon(serializeAddon);
    term.loadAddon(new CanvasAddon());
    term.loadAddon(new WebLinksAddon());

    term.open(containerRef.current);
    term.focus();

    termRef.current = term;
    fitAddonRef.current = fitAddon;
    searchAddonRef.current = searchAddon;
    serializeAddonRef.current = serializeAddon;
    let searchState = { query: "", matchCount: 0, currentIndex: 0 };

    // Use SearchAddon's onDidChangeResults for accurate match counts
    searchAddon.onDidChangeResults((e) => {
      if (e) {
        searchState = {
          ...searchState,
          matchCount: e.resultCount,
          currentIndex: e.resultIndex,
        };
      }
    });

    let fitFrame = 0;
    let fitAttempts = 0;
    const fitWhenReady = () => {
      const container = containerRef.current;
      if (!container) return;

      const rect = container.getBoundingClientRect();
      if ((rect.width < 24 || rect.height < 24) && fitAttempts < 12) {
        fitAttempts += 1;
        fitFrame = window.requestAnimationFrame(fitWhenReady);
        return;
      }

      fitAttempts = 0;
      fitAddon.fit();
      sendResize();
    };

    fitFrame = window.requestAnimationFrame(fitWhenReady);

    const unregisterTerminalController = registerTerminalController(paneId, {
      sendText: (text, options) => sendTextInput(text, options),
      getSnapshot: () => stripAnsi(serializeAddon.serialize()),
      search: (query, direction = "next", reset = false, searchOptions) => {
        const normalizedQuery = query.trim();
        if (!normalizedQuery) {
          searchState = { query: "", matchCount: 0, currentIndex: 0 };
          searchAddon.clearDecorations();
          return searchState;
        }

        const shouldReset = reset || searchState.query !== normalizedQuery;
        const options = {
          incremental: shouldReset,
          regex: searchOptions?.regex ?? false,
          caseSensitive: searchOptions?.caseSensitive ?? false,
          decorations: {
            activeMatchBackground: "#f59e0b",
            matchBackground: "rgba(245, 158, 11, 0.28)",
            matchOverviewRuler: "rgba(245, 158, 11, 0.45)",
            activeMatchColorOverviewRuler: "#f59e0b",
          },
        };

        // findNext/findPrev triggers onDidChangeResults which updates searchState
        const found = direction === "prev"
          ? searchAddon.findPrevious(normalizedQuery, options)
          : searchAddon.findNext(normalizedQuery, options);

        const bufferSnapshot = getSearchableBufferText(term);
        const serializedSnapshot = stripAnsi(serializeAddon.serialize());
        const renderedSnapshot = getRenderedTerminalText(containerRef.current);

        const bufferCount = countSearchMatches(bufferSnapshot, normalizedQuery, searchOptions);
        const serializedCount = countSearchMatches(serializedSnapshot, normalizedQuery, searchOptions);
        const renderedCount = countSearchMatches(renderedSnapshot, normalizedQuery, searchOptions);
        const matchCount = Math.max(bufferCount, serializedCount, renderedCount, found ? 1 : 0);
        let currentIndex = searchState.currentIndex;

        if (shouldReset) {
          currentIndex = matchCount > 0 ? 0 : 0;
        } else if (matchCount > 0) {
          if (direction === "prev") {
            currentIndex = (currentIndex - 1 + matchCount) % matchCount;
          } else {
            currentIndex = (currentIndex + 1) % matchCount;
          }
        } else {
          currentIndex = 0;
        }

        searchState = {
          query: normalizedQuery,
          matchCount,
          currentIndex,
        };
        return searchState;
      },
      clearSearch: () => {
        searchState = { query: "", matchCount: 0, currentIndex: 0 };
        searchAddon.clearDecorations();
      },
      searchHistory: async (query, limit = 8) => {
        const amux = (window as any).tamux ?? (window as any).amux;
        if (!amux?.searchManagedHistory || !sessionReadyRef.current) return false;
        await amux.searchManagedHistory(paneId, query, limit);
        return true;
      },
      generateSkill: async (query, title) => {
        const amux = (window as any).tamux ?? (window as any).amux;
        if (!amux?.generateManagedSkill || !sessionReadyRef.current) return false;
        await amux.generateManagedSkill(paneId, query ?? null, title ?? null);
        return true;
      },
      findSymbol: async (workspaceRoot, symbol, limit = 16) => {
        const amux = (window as any).tamux ?? (window as any).amux;
        if (!amux?.findManagedSymbol || !sessionReadyRef.current) return false;
        await amux.findManagedSymbol(paneId, workspaceRoot, symbol, limit);
        return true;
      },
      listSnapshots: async (workspaceId) => {
        const amux = (window as any).tamux ?? (window as any).amux;
        if (!amux?.listSnapshots || !sessionReadyRef.current) return false;
        await amux.listSnapshots(paneId, workspaceId ?? null);
        return true;
      },
      restoreSnapshot: async (snapshotId) => {
        const amux = (window as any).tamux ?? (window as any).amux;
        if (!amux?.restoreSnapshot || !sessionReadyRef.current) return false;
        await amux.restoreSnapshot(paneId, snapshotId);
        return true;
      },
    });

    term.attachCustomKeyEventHandler((event) => {
      if (event.type !== "keydown") return true;

      const keyboardEvent = event as KeyboardEvent;
      const ctrlOrMeta = keyboardEvent.ctrlKey || keyboardEvent.metaKey;
      const key = keyboardEvent.key.toLowerCase();

      if (ctrlOrMeta && key === "c" && term.hasSelection()) {
        keyboardEvent.preventDefault();
        void copySelection();
        return false;
      }

      if ((ctrlOrMeta && key === "v") || (keyboardEvent.shiftKey && key === "insert")) {
        keyboardEvent.preventDefault();
        void pasteClipboard();
        return false;
      }

      if (ctrlOrMeta && key === "insert") {
        keyboardEvent.preventDefault();
        void copySelection();
        return false;
      }

      return true;
    });

    term.onBell(() => {
      if (settings.visualBell && wrapperRef.current) {
        const wrapper = wrapperRef.current;
        const previousBoxShadow = wrapper.style.boxShadow;
        wrapper.style.boxShadow = "inset 0 0 0 9999px rgba(255,255,255,0.08)";
        window.setTimeout(() => {
          if (wrapperRef.current === wrapper) {
            wrapper.style.boxShadow = previousBoxShadow;
          }
        }, 120);
      }

      if (settings.bellSound) {
        try {
          const audioContext = new (window.AudioContext || (window as typeof window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext)();
          const oscillator = audioContext.createOscillator();
          const gainNode = audioContext.createGain();
          oscillator.type = "sine";
          oscillator.frequency.value = 880;
          gainNode.gain.value = 0.015;
          oscillator.connect(gainNode);
          gainNode.connect(audioContext.destination);
          oscillator.start();
          oscillator.stop(audioContext.currentTime + 0.06);
          window.setTimeout(() => {
            void audioContext.close();
          }, 120);
        } catch {
          // Ignore audio failures in restricted environments.
        }
      }
    });

    term.onSelectionChange(() => {
      if (autoCopyOnSelectRef.current && term.hasSelection()) {
        void copySelection();
      }
    });

    const textarea = (term as Terminal & { textarea?: HTMLTextAreaElement }).textarea;

    const handleNativeCopy = (event: ClipboardEvent) => {
      const selection = term.getSelection();
      if (!selection) return;

      event.preventDefault();
      event.clipboardData?.setData("text/plain", selection);
      void writeClipboardText(selection);
      term.clearSelection();
    };

    const handleNativePaste = (event: ClipboardEvent) => {
      const text = event.clipboardData?.getData("text/plain") ?? "";
      if (!text) return;

      event.preventDefault();
      void sendTextInput(text, { bracketed: true, trackHistory: true });
    };

    let cancelled = false;
    let cleanupTerminalSubscription: (() => void) | undefined;
    let rollingSnapshotTimeout: number | undefined;

    const scheduleRollingSnapshot = () => {
      clearTimeout(rollingSnapshotTimeout);
      rollingSnapshotTimeout = window.setTimeout(() => {
        captureRollingTranscript();
      }, 300);
    };

    void (async () => {
      try {
        const amux = (window as any).tamux ?? (window as any).amux;
        const unsubscribe = amux?.onTerminalEvent?.((event: any) => {
          if (event?.paneId !== paneId || cancelled) return;

          if (event.type === "ready") {
            sessionReadyRef.current = true;
            setDaemonState("reachable");
            setSharedCursorMode("idle");
            requestedSessionIdRef.current = event.sessionId;
            setPaneSessionId(paneId, event.sessionId);
            recordSessionReady({
              paneId,
              workspaceId: paneWorkspaceId ?? null,
              surfaceId: paneSurfaceId ?? null,
              sessionId: event.sessionId,
            });
            fitAttempts = 0;
            fitFrame = window.requestAnimationFrame(fitWhenReady);
            return;
          }

          if (event.type === "output") {
            term.write(decodeBase64ToBytes(event.data));
            recordCognitiveOutput({
              paneId,
              workspaceId: paneWorkspaceId ?? null,
              surfaceId: paneSurfaceId ?? null,
              sessionId: event.sessionId ?? requestedSessionIdRef.current ?? null,
              text: decodeBase64ToText(event.data),
            });
            scheduleRollingSnapshot();
            return;
          }

          if (event.type === "approval-required") {
            setSharedCursorMode("approval");
            const approval = event.approval;
            const approvalId = approval.approvalId ?? approval.approval_id;
            approvalCommandByIdRef.current[approvalId] = approval.command;
            addCommandLogEntry({
              command: approval.command,
              path: "approval-required",
              cwd: paneWorkspaceCwd ?? null,
              workspaceId: paneWorkspaceId ?? null,
              surfaceId: paneSurfaceId ?? null,
              paneId,
            });
            upsertDaemonApproval({
              id: approvalId,
              paneId,
              workspaceId: paneWorkspaceId ?? null,
              surfaceId: paneSurfaceId ?? null,
              sessionId: event.sessionId ?? requestedSessionIdRef.current ?? null,
              command: approval.command,
              reasons: approval.reasons ?? [],
              riskLevel: approval.riskLevel ?? approval.risk_level ?? "high",
              blastRadius: approval.blastRadius ?? approval.blast_radius ?? "current session",
            });
            return;
          }

          if (event.type === "approval-resolved") {
            setSharedCursorMode("idle");
            const approvalId = String(event.approvalId ?? "");
            const command = approvalCommandByIdRef.current[approvalId] ?? `approval ${approvalId}`;
            const decision = String(event.decision ?? "unknown");
            addCommandLogEntry({
              command,
              path: `approval-${decision}`,
              cwd: paneWorkspaceCwd ?? null,
              workspaceId: paneWorkspaceId ?? null,
              surfaceId: paneSurfaceId ?? null,
              paneId,
            });
            delete approvalCommandByIdRef.current[approvalId];
            if (approvalId) {
              markApprovalHandled(approvalId);
            }
            return;
          }

          if (event.type === "managed-started") {
            setSharedCursorMode(event.source === "human" ? "human" : "agent");
            return;
          }

          if (event.type === "managed-queued") {
            const command = String(event.snapshot?.command ?? "").trim();
            if (command) {
              addCommandLogEntry({
                command,
                path: "managed-queued",
                cwd: paneWorkspaceCwd ?? null,
                workspaceId: paneWorkspaceId ?? null,
                surfaceId: paneSurfaceId ?? null,
                paneId,
              });
              recordCommandStarted({
                paneId,
                workspaceId: paneWorkspaceId ?? null,
                surfaceId: paneSurfaceId ?? null,
                sessionId: event.sessionId ?? requestedSessionIdRef.current ?? null,
                command,
              });
            }
            return;
          }

          if (event.type === "managed-finished") {
            setSharedCursorMode("idle");
            if (event.snapshot) {
              setSnapshots([
                {
                  snapshotId: event.snapshot.snapshotId ?? event.snapshot.snapshot_id,
                  workspaceId: event.snapshot.workspaceId ?? event.snapshot.workspace_id ?? null,
                  sessionId: event.snapshot.sessionId ?? event.snapshot.session_id ?? null,
                  command: event.snapshot.command ?? null,
                  kind: event.snapshot.kind,
                  label: event.snapshot.label,
                  path: event.snapshot.path,
                  createdAt: event.snapshot.createdAt ?? event.snapshot.created_at ?? Date.now(),
                  status: event.snapshot.status,
                  details: event.snapshot.details,
                },
                ...useAgentMissionStore.getState().snapshots,
              ]);
            }
            return;
          }

          if (event.type === "managed-rejected") {
            setSharedCursorMode("idle");
            recordError({
              paneId,
              workspaceId: paneWorkspaceId ?? null,
              surfaceId: paneSurfaceId ?? null,
              sessionId: event.sessionId ?? requestedSessionIdRef.current ?? null,
              message: event.message,
            });
            return;
          }

          if (event.type === "history-search-result") {
            setHistoryResults(event.summary ?? "", event.hits ?? []);
            return;
          }

          if (event.type === "symbol-search-result") {
            setSymbolHits(event.matches ?? []);
            return;
          }

          if (event.type === "snapshot-list") {
            setSnapshots((event.snapshots ?? []).map((snapshot: any) => ({
              snapshotId: snapshot.snapshotId ?? snapshot.snapshot_id,
              workspaceId: snapshot.workspaceId ?? snapshot.workspace_id ?? null,
              sessionId: snapshot.sessionId ?? snapshot.session_id ?? null,
              command: snapshot.command ?? null,
              kind: snapshot.kind,
              label: snapshot.label,
              path: snapshot.path,
              createdAt: snapshot.createdAt ?? snapshot.created_at ?? Date.now(),
              status: snapshot.status,
              details: snapshot.details,
            })));
            return;
          }

          if (event.type === "snapshot-restored") {
            const message = event.ok ? event.message : `Restore failed: ${event.message}`;
            term.writeln(`\r\n${message}`);
            return;
          }

          if (event.type === "session-exited") {
            completeLatestPendingEntry({
              paneId,
              exitCode: event.exitCode ?? null,
              finishedAt: Date.now(),
            });
            sessionReadyRef.current = false;
            setDaemonState("unavailable");
            recordSessionExited({
              paneId,
              workspaceId: paneWorkspaceId ?? null,
              surfaceId: paneSurfaceId ?? null,
              sessionId: event.sessionId ?? requestedSessionIdRef.current ?? null,
              exitCode: event.exitCode ?? null,
            });
            term.writeln(`\r\nSession exited${event.exitCode === null || event.exitCode === undefined ? "" : ` (code: ${event.exitCode})`}.`);
            return;
          }

          if (event.type === "command-started") {
            const command = decodeBase64ToText(event.commandB64 ?? "").trim();
            if (!command) return;

            lastShellCommandRef.current = {
              command,
              timestamp: Date.now(),
            };

            commandBufferRef.current = "";
            addCommandLogEntry({
              command,
              path: "shell-start",
              cwd: paneWorkspaceCwd ?? null,
              workspaceId: paneWorkspaceId ?? null,
              surfaceId: paneSurfaceId ?? null,
              paneId,
            });
            recordCommandStarted({
              paneId,
              workspaceId: paneWorkspaceId ?? null,
              surfaceId: paneSurfaceId ?? null,
              sessionId: event.sessionId ?? requestedSessionIdRef.current ?? null,
              command,
            });
            return;
          }

          if (event.type === "command-finished") {
            completeLatestPendingEntry({
              paneId,
              exitCode: event.exitCode ?? null,
              finishedAt: Date.now(),
            });
            recordCommandFinished({
              paneId,
              workspaceId: paneWorkspaceId ?? null,
              surfaceId: paneSurfaceId ?? null,
              sessionId: event.sessionId ?? requestedSessionIdRef.current ?? null,
              command: lastShellCommandRef.current?.command ?? null,
              exitCode: event.exitCode ?? null,
              durationMs: lastShellCommandRef.current ? Math.max(0, Date.now() - lastShellCommandRef.current.timestamp) : null,
            });
            return;
          }

          if (event.type === "error") {
            sessionReadyRef.current = false;
            setDaemonState("unavailable");
            recordError({
              paneId,
              workspaceId: paneWorkspaceId ?? null,
              surfaceId: paneSurfaceId ?? null,
              sessionId: requestedSessionIdRef.current ?? null,
              message: event.message,
            });
            term.writeln(`\r\n\x1b[31m${event.message}\x1b[0m`);
          }
        });

        const shell = settings.defaultShell.trim() || undefined;
        const bridge = await amux?.startTerminalSession?.({
          paneId,
          sessionId: requestedSessionIdRef.current,
          shell,
          cwd: paneWorkspaceCwd || undefined,
          workspaceId: paneWorkspaceId,
          cols: term.cols,
          rows: term.rows,
        });

        if (cancelled) return;

        if (Array.isArray(bridge?.initialOutput)) {
          term.clear();
          for (const chunk of bridge.initialOutput) {
            term.write(decodeBase64ToBytes(chunk));
          }
          scheduleRollingSnapshot();
        }

        if (typeof bridge?.sessionId === "string" && bridge.sessionId) {
          requestedSessionIdRef.current = bridge.sessionId;
          setPaneSessionId(paneId, bridge.sessionId);
        }

        if (bridge?.state === "reachable") {
          sessionReadyRef.current = true;
          setDaemonState("reachable");
          fitAttempts = 0;
          fitFrame = window.requestAnimationFrame(fitWhenReady);
        } else {
          sessionReadyRef.current = false;
          setDaemonState("checking");
        }

        if (typeof unsubscribe === "function") {
          cleanupTerminalSubscription = unsubscribe;
        }
      } catch {
        if (cancelled) return;
        setDaemonState("unavailable");
        term.writeln("\x1b[31mDaemon check failed.\x1b[0m");
      }
    })();

    term.onData((data) => {
      const amux = (window as any).tamux ?? (window as any).amux;
      if (!amux?.sendTerminalInput || !sessionReadyRef.current) return;

      if (!pendingApprovalIdRef.current) {
        let command = "";
        if (data === "\r" || data === "\n") {
          command = commandBufferRef.current.trim();
        } else {
          const newlineIndex = data.search(/[\r\n]/);
          if (newlineIndex >= 0) {
            // Handle paste events where command and Enter arrive in one chunk.
            command = `${commandBufferRef.current}${data.slice(0, newlineIndex)}`.trim();
          }
        }

        if (command) {
          setSharedCursorMode("human");
          const sessionKey = requestedSessionIdRef.current ?? paneId;
          const risk = assessCommandRisk(command, settings.securityLevel);

          if (risk.requiresApproval && !isCommandAllowed(sessionKey, command)) {
            pendingApprovalIdRef.current = requestApproval({
              paneId,
              workspaceId: paneWorkspaceId ?? null,
              surfaceId: paneSurfaceId ?? null,
              sessionId: requestedSessionIdRef.current ?? null,
              command,
              reasons: risk.reasons,
              riskLevel: risk.riskLevel,
              blastRadius: risk.blastRadius,
            });
            return;
          }
        }
      }

      setSharedCursorMode("human");
      trackInput(data);
      void amux.sendTerminalInput(paneId, encodeTextToBase64(data));
    });

    const handleContextMenu = (event: MouseEvent) => {
      event.preventDefault();
      handleFocus();

      const menuWidth = 220;
      const menuHeight = 286;
      const x = Math.min(event.clientX, Math.max(window.innerWidth - menuWidth - 8, 8));
      const y = Math.min(event.clientY, Math.max(window.innerHeight - menuHeight - 8, 8));

      setContextMenu({ visible: true, x, y });
    };

    const handleDragOver = (event: DragEvent) => {
      if (!event.dataTransfer) return;
      if (!Array.from(event.dataTransfer.types).includes("Files")) return;

      event.preventDefault();
      event.dataTransfer.dropEffect = "copy";
      handleFocus();
    };

    const handleDrop = (event: DragEvent) => {
      const transfer = event.dataTransfer;
      if (!transfer) return;

      event.preventDefault();
      hideContextMenu();
      handleFocus();

      const files = Array.from(transfer.files ?? []);
      if (files.length > 0) {
        const payload = files
          .map((file) => {
            const filePath = (file as File & { path?: string }).path || file.name;
            return filePath ? quotePathForShell(filePath, platformRef.current) : "";
          })
          .filter(Boolean)
          .join(" ");

        if (payload) {
          void sendTextInput(`${payload} `, { trackHistory: false });
        }
        return;
      }

      const text = transfer.getData("text/plain");
      if (text) {
        void sendTextInput(text, { bracketed: true, trackHistory: false });
      }
    };

    const handleWindowPointer = () => hideContextMenu();
    const handleWindowKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") hideContextMenu();
    };

    const appCommandUnsubscribe = ((window as any).tamux ?? (window as any).amux)?.onAppCommand?.((command: string) => {
      if (useWorkspaceStore.getState().activePaneId() !== paneId) return;

      switch (command) {
        case "copy":
          void copySelection();
          break;
        case "paste":
          void pasteClipboard();
          break;
        case "select-all":
          term.selectAll();
          break;
      }
    });

    const wrapper = wrapperRef.current;
    wrapper?.addEventListener("contextmenu", handleContextMenu);
    wrapper?.addEventListener("dragover", handleDragOver);
    wrapper?.addEventListener("drop", handleDrop);
    textarea?.addEventListener("copy", handleNativeCopy);
    textarea?.addEventListener("cut", handleNativeCopy);
    textarea?.addEventListener("paste", handleNativePaste);
    window.addEventListener("pointerdown", handleWindowPointer);
    window.addEventListener("blur", handleWindowPointer);
    window.addEventListener("keydown", handleWindowKeyDown);

    // Auto-fit on container resize.
    const observer = new ResizeObserver(() => {
      // Debounce resize to avoid flooding ConPTY (see architecture notes).
      clearTimeout(resizeTimeout);
      resizeTimeout = window.setTimeout(() => {
        fitAttempts = 0;
        fitFrame = window.requestAnimationFrame(fitWhenReady);
      }, 50);
    });

    let resizeTimeout: number;
    observer.observe(containerRef.current);

    return () => {
      cancelled = true;
      sessionReadyRef.current = false;
      window.cancelAnimationFrame(fitFrame);
      clearTimeout(rollingSnapshotTimeout);
      clearTimeout(resizeTimeout);
      observer.disconnect();
      unregisterTerminalController();
      cleanupTerminalSubscription?.();
      appCommandUnsubscribe?.();
      wrapper?.removeEventListener("contextmenu", handleContextMenu);
      wrapper?.removeEventListener("dragover", handleDragOver);
      wrapper?.removeEventListener("drop", handleDrop);
      textarea?.removeEventListener("copy", handleNativeCopy);
      textarea?.removeEventListener("cut", handleNativeCopy);
      textarea?.removeEventListener("paste", handleNativePaste);
      window.removeEventListener("pointerdown", handleWindowPointer);
      window.removeEventListener("blur", handleWindowPointer);
      window.removeEventListener("keydown", handleWindowKeyDown);
      searchAddonRef.current = null;
      serializeAddonRef.current = null;
      term.dispose();
    };
  }, [paneId, settings.themeName, settings.fontFamily, settings.fontSize,
    settings.cursorBlink, settings.cursorStyle, settings.lineHeight, settings.scrollbackLines,
    settings.bellSound, settings.visualBell, settings.padding,
    settings.useCustomTerminalColors, settings.customTerminalBackground,
    settings.customTerminalForeground, settings.customTerminalCursor,
    settings.customTerminalSelection, copySelection,
    pasteClipboard, hideContextMenu, handleFocus, setPaneSessionId,
    paneWorkspaceCwd, paneWorkspaceId, paneSurfaceId, settings.defaultShell, sendResize, sendTextInput,
    writeClipboardText, captureRollingTranscript, trackInput, completeLatestPendingEntry, addCommandLogEntry,
    recordCommandFinished, recordCommandStarted, recordCognitiveOutput, recordError, recordSessionExited, recordSessionReady,
    isCommandAllowed, requestApproval, setHistoryResults, setSharedCursorMode, setSnapshots, setSymbolHits, upsertDaemonApproval]);

  const isActive = activePaneId() === paneId;
  const canCopy = Boolean(termRef.current?.hasSelection());
  const canPaste = daemonState === "reachable";
  const menuItems = buildTerminalContextMenuItems({
    canCopy,
    canPaste,
    copySelection,
    pasteClipboard,
    termRef,
    splitActive,
    toggleZoom,
    handleClosePane,
    settings,
    captureTranscript,
    paneId,
    sendRawFormFeed: (currentPaneId) => {
      void (((window as any).tamux ?? (window as any).amux)?.sendTerminalInput?.(currentPaneId, encodeTextToBase64("\f")));
    },
    toggleSearch,
  });

  return (
    <div
      ref={wrapperRef}
      onFocus={handleFocus}
      onClick={handleFocus}
      tabIndex={0}
      style={{
        width: "100%",
        height: "100%",
        background: "var(--bg-primary)",
        padding: `${Math.max(12, settings.padding)}px`,
        borderLeft: isActive ? "2px solid var(--accent)" : "2px solid transparent",
        position: "relative",
        outline: "none",
      }}
    >
      <TerminalPaneHeader
        paneId={paneId}
        paneName={paneName}
        paneNameDraft={paneNameDraft}
        setPaneNameDraft={setPaneNameDraft}
        setPaneName={setPaneName}
      />

      <div ref={containerRef} style={{ width: "100%", height: "calc(100% - 36px)" }} />
      <SharedCursor mode={sharedCursorMode} />

      <TerminalContextMenu
        visible={contextMenu.visible}
        x={contextMenu.x}
        y={contextMenu.y}
        items={menuItems}
        hideContextMenu={hideContextMenu}
      />
    </div>
  );
}
