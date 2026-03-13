import { useEffect } from "react";
import { useHotkeys } from "../../hooks/useHotkeys";
import { saveSession, startAutoSave } from "../../lib/sessionPersistence";
import { useSettingsStore } from "../../lib/settingsStore";
import { applyAppShellTheme, getAppShellTheme } from "../../lib/themes";
import { useWorkspaceStore } from "../../lib/workspaceStore";

export const AppRuntimeBridge: React.FC = () => {
    const createWorkspace = useWorkspaceStore((s) => s.createWorkspace);
    const createSurface = useWorkspaceStore((s) => s.createSurface);
    const splitActive = useWorkspaceStore((s) => s.splitActive);
    const toggleZoom = useWorkspaceStore((s) => s.toggleZoom);
    const toggleSidebar = useWorkspaceStore((s) => s.toggleSidebar);
    const toggleSettings = useWorkspaceStore((s) => s.toggleSettings);
    const toggleSearch = useWorkspaceStore((s) => s.toggleSearch);
    const toggleFileManager = useWorkspaceStore((s) => s.toggleFileManager);
    const toggleCommandPalette = useWorkspaceStore((s) => s.toggleCommandPalette);
    const toggleCommandHistory = useWorkspaceStore((s) => s.toggleCommandHistory);
    const toggleCommandLog = useWorkspaceStore((s) => s.toggleCommandLog);
    const toggleSystemMonitor = useWorkspaceStore((s) => s.toggleSystemMonitor);
    const toggleCanvas = useWorkspaceStore((s) => s.toggleCanvas);
    const toggleTimeTravel = useWorkspaceStore((s) => s.toggleTimeTravel);
    const toggleAgentPanel = useWorkspaceStore((s) => s.toggleAgentPanel);
    const toggleSessionVault = useWorkspaceStore((s) => s.toggleSessionVault);
    const workspaces = useWorkspaceStore((s) => s.workspaces);
    const sidebarVisible = useWorkspaceStore((s) => s.sidebarVisible);
    const sidebarWidth = useWorkspaceStore((s) => s.sidebarWidth);
    const settings = useSettingsStore((s) => s.settings);
    const settingsOpen = useWorkspaceStore((s) => s.settingsOpen);

    useHotkeys();

    useEffect(() => {
        if (workspaces.length === 0) {
            createWorkspace("Default");
        }
    }, [createWorkspace, workspaces.length]);

    useEffect(() => startAutoSave(30_000), []);

    useEffect(() => {
        const timeoutId = window.setTimeout(() => {
            saveSession();
        }, 500);

        return () => window.clearTimeout(timeoutId);
    }, [sidebarVisible, sidebarWidth, workspaces]);

    useEffect(() => {
        applyAppShellTheme(
            getAppShellTheme(
                settings.themeName,
                settings.useCustomTerminalColors,
                settings.customTerminalBackground,
                settings.customTerminalForeground,
                settings.customTerminalCursor,
                settings.customTerminalSelection,
            ),
        );

        const amux = (window as any).tamux ?? (window as any).amux;
        void amux?.setWindowOpacity?.(settings.opacity);
    }, [
        settings.customTerminalBackground,
        settings.customTerminalCursor,
        settings.customTerminalForeground,
        settings.customTerminalSelection,
        settings.opacity,
        settings.themeName,
        settings.useCustomTerminalColors,
    ]);

    useEffect(() => {
        const amux = (window as any).tamux ?? (window as any).amux;
        if (!amux?.onAppCommand) return;

        return amux.onAppCommand((command: string) => {
            switch (command) {
                case "new-workspace":
                    createWorkspace();
                    break;
                case "new-surface":
                    createSurface();
                    break;
                case "toggle-settings":
                    toggleSettings();
                    break;
                case "toggle-command-palette":
                    toggleCommandPalette();
                    break;
                case "toggle-search":
                    toggleSearch();
                    break;
                case "toggle-file-manager":
                    toggleFileManager();
                    break;
                case "toggle-mission":
                    toggleAgentPanel();
                    break;
                case "toggle-command-history":
                    toggleCommandHistory();
                    break;
                case "toggle-command-log":
                    toggleCommandLog();
                    break;
                case "toggle-session-vault":
                    toggleSessionVault();
                    break;
                case "toggle-system-monitor":
                    toggleSystemMonitor();
                    break;
                case "toggle-canvas":
                    toggleCanvas();
                    break;
                case "toggle-time-travel":
                    toggleTimeTravel();
                    break;
                case "toggle-sidebar":
                    toggleSidebar();
                    break;
                case "split-right":
                    splitActive("horizontal");
                    break;
                case "split-down":
                    splitActive("vertical");
                    break;
                case "toggle-zoom":
                    toggleZoom();
                    break;
                case "about":
                    if (!settingsOpen) {
                        toggleSettings();
                    }
                    window.setTimeout(() => {
                        window.dispatchEvent(new CustomEvent("tamux-open-settings-tab", {
                            detail: { tab: "about" },
                        }));
                        window.dispatchEvent(new CustomEvent("amux-open-settings-tab", {
                            detail: { tab: "about" },
                        }));
                    }, 50);
                    break;
            }
        });
    }, [
        createSurface,
        createWorkspace,
        settingsOpen,
        splitActive,
        toggleAgentPanel,
        toggleCanvas,
        toggleCommandHistory,
        toggleCommandLog,
        toggleCommandPalette,
        toggleFileManager,
        toggleSearch,
        toggleSessionVault,
        toggleSettings,
        toggleSidebar,
        toggleSystemMonitor,
        toggleTimeTravel,
        toggleZoom,
    ]);

    return null;
};