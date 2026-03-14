/**
 * Core type definitions for the amux application.
 * Maps to the amux-windows data models: Workspace, Surface, Notification,
 * CommandLog, Settings, ShellProfile, Transcript.
 */

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------
export type WorkspaceId = string;
export type SurfaceId = string;
export type PaneId = string;
export type SessionId = string;
export type NotificationId = string;

// ---------------------------------------------------------------------------
// Infinite canvas
// ---------------------------------------------------------------------------
export type SurfaceLayoutMode = "bsp" | "canvas";
export type CanvasPanelStatus = "running" | "idle" | "needs_approval";

export interface CanvasViewSnapshot {
  panX: number;
  panY: number;
  zoomLevel: number;
}

export interface CanvasState extends CanvasViewSnapshot {
  previousView: CanvasViewSnapshot | null;
  focusRequestNonce?: number;
}

export interface CanvasPanel {
  id: string;
  paneId: PaneId;
  title: string;
  icon: string;
  x: number;
  y: number;
  width: number;
  height: number;
  status: CanvasPanelStatus;
  sessionId: SessionId | null;
  lastActivityAt: number;
}

// ---------------------------------------------------------------------------
// Workspace
// ---------------------------------------------------------------------------
export interface Workspace {
  id: WorkspaceId;
  name: string;
  icon: string;
  accentColor: string;
  cwd: string;
  gitBranch: string | null;
  gitDirty: boolean;
  listeningPorts: number[];
  unreadCount: number;
  surfaces: Surface[];
  activeSurfaceId: SurfaceId | null;
  createdAt: number;
}

// ---------------------------------------------------------------------------
// Surface (Tab)
// ---------------------------------------------------------------------------
export interface Surface {
  id: SurfaceId;
  workspaceId: WorkspaceId;
  name: string;
  icon: string;
  layoutMode: SurfaceLayoutMode;
  layout: import("./bspTree").BspTree;
  paneNames: Record<PaneId, string>;
  paneIcons: Record<PaneId, string>;
  activePaneId: PaneId | null;
  canvasState: CanvasState;
  canvasPanels: CanvasPanel[];
  createdAt: number;
}

// ---------------------------------------------------------------------------
// Pane metadata (tracked alongside BSP leaves)
// ---------------------------------------------------------------------------
export interface PaneInfo {
  id: PaneId;
  sessionId: SessionId | null;
  cwd: string | null;
  title: string | null;
  isZoomed: boolean;
}

// ---------------------------------------------------------------------------
// Notifications (OSC 9, 99, 777)
// ---------------------------------------------------------------------------
export type NotificationSource = "osc9" | "osc99" | "osc777" | "cli" | "system" | "approval";

export interface TerminalNotification {
  id: NotificationId;
  workspaceId: WorkspaceId | null;
  surfaceId: SurfaceId | null;
  paneId: PaneId | null;
  panelId?: PaneId | null;
  title: string;
  subtitle: string | null;
  body: string;
  icon: string | null;
  progress: number | null; // 0..100
  isRead: boolean;
  timestamp: number;
  source: NotificationSource;
}

// ---------------------------------------------------------------------------
// Command Log
// ---------------------------------------------------------------------------
export interface CommandLogEntry {
  id: string;
  command: string;
  timestamp: number;
  path: string | null;
  cwd: string | null;
  workspaceId: WorkspaceId | null;
  surfaceId: SurfaceId | null;
  paneId: PaneId | null;
  exitCode: number | null;
  durationMs: number | null;
}

// ---------------------------------------------------------------------------
// Transcript
// ---------------------------------------------------------------------------
export type TranscriptReason =
  | "live"
  | "pane-close"
  | "workspace-close"
  | "surface-close"
  | "terminal-clear"
  | "manual";

export interface TranscriptEntry {
  id: string;
  filename: string;
  filePath: string;
  reason: TranscriptReason;
  workspaceId: WorkspaceId | null;
  surfaceId: SurfaceId | null;
  paneId: PaneId | null;
  cwd: string | null;
  capturedAt: number;
  sizeBytes: number;
  preview: string; // first 500 chars
  content: string;
}

// ---------------------------------------------------------------------------
// Settings (mirrors AmuxConfig in Rust)
// ---------------------------------------------------------------------------
export interface AmuxSettings {
  // Appearance
  fontFamily: string;
  fontSize: number;
  themeName: string;
  useCustomTerminalColors: boolean;
  customTerminalBackground: string;
  customTerminalForeground: string;
  customTerminalCursor: string;
  customTerminalSelection: string;
  opacity: number;
  lineHeight: number;
  padding: number;

  // Cursor
  cursorStyle: "bar" | "block" | "underline";
  cursorBlink: boolean;
  cursorBlinkMs: number;

  // Terminal
  defaultShell: string;
  defaultShellArgs: string;
  scrollbackLines: number;
  bellSound: boolean;
  visualBell: boolean;
  bracketedPaste: boolean;

  // Behavior
  restoreSessionOnStartup: boolean;
  confirmOnClose: boolean;
  autoCopyOnSelect: boolean;
  ctrlClickOpensUrls: boolean;
  autoSaveIntervalSeconds: number;
  captureTranscriptsOnClose: boolean;
  captureTranscriptsOnClear: boolean;
  commandLogRetentionDays: number;
  transcriptRetentionDays: number;

  // Infrastructure
  securityLevel: "highest" | "moderate" | "lowest" | "yolo";
  sandboxEnabled: boolean;
  sandboxNetworkEnabled: boolean;
  snapshotBackend: "tar" | "zfs" | "btrfs";
  wormIntegrityEnabled: boolean;
  cerbosEndpoint: string;
  mcpServersJson: string;

  // Gateway
  gatewayEnabled: boolean;
  slackToken: string;
  slackChannelFilter: string;
  telegramToken: string;
  telegramAllowedChats: string;
  discordToken: string;
  discordChannelFilter: string;
  discordAllowedUsers: string;
  whatsappToken: string;
  whatsappPhoneNumberId: string;
  whatsappAllowedContacts: string;
  gatewayCommandPrefix: string;
}

export const DEFAULT_SETTINGS: AmuxSettings = {
  fontFamily: "Cascadia Code",
  fontSize: 14,
  themeName: "Catppuccin Mocha",
  useCustomTerminalColors: false,
  customTerminalBackground: "",
  customTerminalForeground: "",
  customTerminalCursor: "",
  customTerminalSelection: "",
  opacity: 1.0,
  lineHeight: 1.0,
  padding: 0,
  cursorStyle: "bar",
  cursorBlink: true,
  cursorBlinkMs: 530,
  defaultShell: "",
  defaultShellArgs: "",
  scrollbackLines: 10000,
  bellSound: false,
  visualBell: true,
  bracketedPaste: true,
  restoreSessionOnStartup: true,
  confirmOnClose: true,
  autoCopyOnSelect: false,
  ctrlClickOpensUrls: true,
  autoSaveIntervalSeconds: 30,
  captureTranscriptsOnClose: true,
  captureTranscriptsOnClear: true,
  commandLogRetentionDays: 90,
  transcriptRetentionDays: 90,
  securityLevel: "moderate",
  sandboxEnabled: false,
  sandboxNetworkEnabled: true,
  snapshotBackend: "tar",
  wormIntegrityEnabled: true,
  cerbosEndpoint: "",
  mcpServersJson: "{\n  \"tamux\": {\n    \"command\": \"tamux-mcp\"\n  }\n}",
  gatewayEnabled: false,
  slackToken: "",
  slackChannelFilter: "",
  telegramToken: "",
  telegramAllowedChats: "",
  discordToken: "",
  discordChannelFilter: "",
  discordAllowedUsers: "",
  whatsappToken: "",
  whatsappPhoneNumberId: "",
  whatsappAllowedContacts: "",
  gatewayCommandPrefix: "!tamux",
};

// ---------------------------------------------------------------------------
// Shell Profile
// ---------------------------------------------------------------------------
export interface ShellProfile {
  id: string;
  name: string;
  command: string;
  args: string[];
  cwd: string;
  env: Record<string, string>;
  themeOverride: string | null;
  isDefault: boolean;
}

// ---------------------------------------------------------------------------
// Session persistence
// ---------------------------------------------------------------------------
export interface PersistedSession {
  version: number;
  windowState: {
    x: number;
    y: number;
    width: number;
    height: number;
    maximized: boolean;
  };
  sidebarVisible: boolean;
  sidebarWidth: number;
  workspaces: PersistedWorkspace[];
  activeWorkspaceId: WorkspaceId | null;
}

export interface PersistedWorkspace {
  id: WorkspaceId;
  name: string;
  icon: string;
  accentColor: string;
  cwd: string;
  browser?: PersistedWorkspaceBrowser;
  surfaces: PersistedSurface[];
  activeSurfaceId: SurfaceId | null;
}

export interface PersistedWorkspaceBrowser {
  open: boolean;
  fullscreen: boolean;
  url: string;
  history: string[];
  historyIndex: number;
}

export interface PersistedSurface {
  id: SurfaceId;
  name: string;
  icon: string;
  layoutMode?: SurfaceLayoutMode;
  layout: import("./bspTree").BspTree;
  activePaneId: PaneId | null;
  paneNames?: Record<PaneId, string>;
  paneIcons?: Record<PaneId, string>;
  canvasState?: PersistedCanvasState;
  canvasPanels?: PersistedCanvasPanel[];
  panes: PersistedPane[];
}

export interface PersistedCanvasState {
  panX: number;
  panY: number;
  zoomLevel: number;
  previousView: CanvasViewSnapshot | null;
}

export interface PersistedCanvasPanel {
  id: string;
  paneId: PaneId;
  title: string;
  icon: string;
  x: number;
  y: number;
  width: number;
  height: number;
  status: CanvasPanelStatus;
  sessionId: SessionId | null;
  lastActivityAt: number;
}

export type HotkeyAction =
  | "splitHorizontal"
  | "splitVertical"
  | "closePane"
  | "toggleZoom"
  | "focusLeft"
  | "focusRight"
  | "focusUp"
  | "focusDown"
  | "newSurface"
  | "closeSurface"
  | "nextSurface"
  | "prevSurface"
  | "newWorkspace"
  | "switchWorkspace1"
  | "switchWorkspace2"
  | "switchWorkspace3"
  | "switchWorkspace4"
  | "switchWorkspace5"
  | "switchWorkspace6"
  | "switchWorkspace7"
  | "switchWorkspace8"
  | "switchWorkspace9"
  | "nextWorkspace"
  | "prevWorkspace"
  | "toggleCommandPalette"
  | "toggleSidebar"
  | "toggleNotifications"
  | "toggleSettings"
  | "toggleSessionVault"
  | "toggleCommandLog"
  | "toggleSearch"
  | "toggleCommandHistory"
  | "toggleSnippets"
  | "toggleAgentPanel"
  | "toggleSystemMonitor"
  | "toggleFileManager"
  | "toggleCanvas"
  | "toggleTimeTravel";

export interface Keybinding {
  action: HotkeyAction;
  combo: string;
  description: string;
}

export interface PersistedPane {
  id: PaneId;
  cwd: string | null;
  scrollback: string | null; // base64 of last N lines
  commandHistory: string[];
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------
export interface SearchState {
  query: string;
  matches: SearchMatch[];
  currentIndex: number;
  isOpen: boolean;
}

export interface SearchMatch {
  paneId: PaneId;
  row: number;
  col: number;
  length: number;
}
