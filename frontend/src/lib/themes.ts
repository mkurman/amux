/**
 * Terminal theme definitions — 8 built-in themes matching amux-windows.
 * Each theme has a 16-color ANSI palette + background/foreground/cursor/selection.
 */

export interface TerminalThemeColors {
  background: string;
  foreground: string;
  cursor: string;
  selectionBg: string;
  selectionFg?: string;
  black: string;
  red: string;
  green: string;
  yellow: string;
  blue: string;
  magenta: string;
  cyan: string;
  white: string;
  brightBlack: string;
  brightRed: string;
  brightGreen: string;
  brightYellow: string;
  brightBlue: string;
  brightMagenta: string;
  brightCyan: string;
  brightWhite: string;
}

export interface TerminalTheme {
  name: string;
  author: string;
  colors: TerminalThemeColors;
}

export type AppShellTheme = Record<string, string>;

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function normalizeHex(hex: string): string {
  const value = hex.trim().replace(/^#/, "");
  if (value.length === 3) {
    return value
      .split("")
      .map((char) => `${char}${char}`)
      .join("");
  }
  return value.padEnd(6, "0").slice(0, 6);
}

function parseHex(hex: string): [number, number, number] {
  const normalized = normalizeHex(hex);
  return [
    Number.parseInt(normalized.slice(0, 2), 16),
    Number.parseInt(normalized.slice(2, 4), 16),
    Number.parseInt(normalized.slice(4, 6), 16),
  ];
}

function toHex(red: number, green: number, blue: number): string {
  return `#${[red, green, blue]
    .map((value) => clamp(Math.round(value), 0, 255).toString(16).padStart(2, "0"))
    .join("")}`;
}

function mix(colorA: string, colorB: string, amount: number): string {
  const [redA, greenA, blueA] = parseHex(colorA);
  const [redB, greenB, blueB] = parseHex(colorB);
  const ratio = clamp(amount, 0, 1);
  return toHex(
    redA + (redB - redA) * ratio,
    greenA + (greenB - greenA) * ratio,
    blueA + (blueB - blueA) * ratio
  );
}

function withAlpha(color: string, alpha: number): string {
  const [red, green, blue] = parseHex(color);
  return `rgba(${red}, ${green}, ${blue}, ${clamp(alpha, 0, 1)})`;
}

export const BUILTIN_THEMES: TerminalTheme[] = [
  {
    name: "Default Dark",
    author: "tamux",
    colors: {
      background: "#1e1e1e",
      foreground: "#cccccc",
      cursor: "#cccccc",
      selectionBg: "#264f78",
      black: "#000000",
      red: "#cd3131",
      green: "#0dbc79",
      yellow: "#e5e510",
      blue: "#2472c8",
      magenta: "#bc3fbc",
      cyan: "#11a8cd",
      white: "#e5e5e5",
      brightBlack: "#666666",
      brightRed: "#f14c4c",
      brightGreen: "#23d18b",
      brightYellow: "#f5f543",
      brightBlue: "#3b8eea",
      brightMagenta: "#d670d6",
      brightCyan: "#29b8db",
      brightWhite: "#e5e5e5",
    },
  },
  {
    name: "Dracula",
    author: "Zeno Rocha",
    colors: {
      background: "#282a36",
      foreground: "#f8f8f2",
      cursor: "#f8f8f2",
      selectionBg: "#44475a",
      black: "#21222c",
      red: "#ff5555",
      green: "#50fa7b",
      yellow: "#f1fa8c",
      blue: "#bd93f9",
      magenta: "#ff79c6",
      cyan: "#8be9fd",
      white: "#f8f8f2",
      brightBlack: "#6272a4",
      brightRed: "#ff6e6e",
      brightGreen: "#69ff94",
      brightYellow: "#ffffa5",
      brightBlue: "#d6acff",
      brightMagenta: "#ff92df",
      brightCyan: "#a4ffff",
      brightWhite: "#ffffff",
    },
  },
  {
    name: "Nord",
    author: "Arctic Ice Studio",
    colors: {
      background: "#2e3440",
      foreground: "#d8dee9",
      cursor: "#d8dee9",
      selectionBg: "#434c5e",
      black: "#3b4252",
      red: "#bf616a",
      green: "#a3be8c",
      yellow: "#ebcb8b",
      blue: "#81a1c1",
      magenta: "#b48ead",
      cyan: "#88c0d0",
      white: "#e5e9f0",
      brightBlack: "#4c566a",
      brightRed: "#bf616a",
      brightGreen: "#a3be8c",
      brightYellow: "#ebcb8b",
      brightBlue: "#81a1c1",
      brightMagenta: "#b48ead",
      brightCyan: "#8fbcbb",
      brightWhite: "#eceff4",
    },
  },
  {
    name: "Solarized Dark",
    author: "Ethan Schoonover",
    colors: {
      background: "#002b36",
      foreground: "#839496",
      cursor: "#839496",
      selectionBg: "#073642",
      black: "#073642",
      red: "#dc322f",
      green: "#859900",
      yellow: "#b58900",
      blue: "#268bd2",
      magenta: "#d33682",
      cyan: "#2aa198",
      white: "#eee8d5",
      brightBlack: "#586e75",
      brightRed: "#cb4b16",
      brightGreen: "#586e75",
      brightYellow: "#657b83",
      brightBlue: "#839496",
      brightMagenta: "#6c71c4",
      brightCyan: "#93a1a1",
      brightWhite: "#fdf6e3",
    },
  },
  {
    name: "One Dark",
    author: "Atom",
    colors: {
      background: "#282c34",
      foreground: "#abb2bf",
      cursor: "#528bff",
      selectionBg: "#3e4451",
      black: "#282c34",
      red: "#e06c75",
      green: "#98c379",
      yellow: "#e5c07b",
      blue: "#61afef",
      magenta: "#c678dd",
      cyan: "#56b6c2",
      white: "#abb2bf",
      brightBlack: "#5c6370",
      brightRed: "#e06c75",
      brightGreen: "#98c379",
      brightYellow: "#e5c07b",
      brightBlue: "#61afef",
      brightMagenta: "#c678dd",
      brightCyan: "#56b6c2",
      brightWhite: "#ffffff",
    },
  },
  {
    name: "Monokai",
    author: "Wimer Hazenberg",
    colors: {
      background: "#272822",
      foreground: "#f8f8f2",
      cursor: "#f8f8f0",
      selectionBg: "#49483e",
      black: "#272822",
      red: "#f92672",
      green: "#a6e22e",
      yellow: "#f4bf75",
      blue: "#66d9ef",
      magenta: "#ae81ff",
      cyan: "#a1efe4",
      white: "#f8f8f2",
      brightBlack: "#75715e",
      brightRed: "#f92672",
      brightGreen: "#a6e22e",
      brightYellow: "#f4bf75",
      brightBlue: "#66d9ef",
      brightMagenta: "#ae81ff",
      brightCyan: "#a1efe4",
      brightWhite: "#f9f8f5",
    },
  },
  {
    name: "Tokyo Night",
    author: "enkia",
    colors: {
      background: "#1a1b26",
      foreground: "#c0caf5",
      cursor: "#c0caf5",
      selectionBg: "#33467c",
      black: "#15161e",
      red: "#f7768e",
      green: "#9ece6a",
      yellow: "#e0af68",
      blue: "#7aa2f7",
      magenta: "#bb9af7",
      cyan: "#7dcfff",
      white: "#a9b1d6",
      brightBlack: "#414868",
      brightRed: "#f7768e",
      brightGreen: "#9ece6a",
      brightYellow: "#e0af68",
      brightBlue: "#7aa2f7",
      brightMagenta: "#bb9af7",
      brightCyan: "#7dcfff",
      brightWhite: "#c0caf5",
    },
  },
  {
    name: "Catppuccin Mocha",
    author: "Catppuccin",
    colors: {
      background: "#1e1e2e",
      foreground: "#cdd6f4",
      cursor: "#f5e0dc",
      selectionBg: "#45475a",
      black: "#45475a",
      red: "#f38ba8",
      green: "#a6e3a1",
      yellow: "#f9e2af",
      blue: "#89b4fa",
      magenta: "#f5c2e7",
      cyan: "#94e2d5",
      white: "#bac2de",
      brightBlack: "#585b70",
      brightRed: "#f38ba8",
      brightGreen: "#a6e3a1",
      brightYellow: "#f9e2af",
      brightBlue: "#89b4fa",
      brightMagenta: "#f5c2e7",
      brightCyan: "#94e2d5",
      brightWhite: "#a6adc8",
    },
  },
];

/** Get a theme by name (case-insensitive). Falls back to Catppuccin Mocha. */
export function getThemeByName(name: string): TerminalTheme {
  return (
    BUILTIN_THEMES.find(
      (t) => t.name.toLowerCase() === name.toLowerCase()
    ) ?? BUILTIN_THEMES[7]
  );
}

/** Build effective theme colors, merging custom overrides on top of the base theme. */
export function getEffectiveTheme(
  themeName: string,
  useCustomColors: boolean,
  customBg?: string,
  customFg?: string,
  customCursor?: string,
  customSelection?: string
): TerminalThemeColors {
  const base = getThemeByName(themeName).colors;
  if (!useCustomColors) return base;
  return {
    ...base,
    ...(customBg ? { background: customBg } : {}),
    ...(customFg ? { foreground: customFg } : {}),
    ...(customCursor ? { cursor: customCursor } : {}),
    ...(customSelection ? { selectionBg: customSelection } : {}),
  };
}

export function getAppShellTheme(
  themeName: string,
  useCustomColors: boolean,
  customBg?: string,
  customFg?: string,
  customCursor?: string,
  customSelection?: string
): AppShellTheme {
  const colors = getEffectiveTheme(
    themeName,
    useCustomColors,
    customBg,
    customFg,
    customCursor,
    customSelection
  );

  // New design system color mapping - consistent structure across all themes
  const bgVoid = mix(colors.background, "#000000", 0.5);
  const bgDeep = mix(colors.background, "#000000", 0.3);
  const bgPrimary = colors.background;
  const bgSecondary = mix(colors.background, colors.black, 0.38);
  const bgTertiary = mix(colors.background, colors.black, 0.2);
  const bgSurface = mix(colors.background, colors.white, 0.1);
  const bgElevated = mix(colors.background, colors.white, 0.16);

  // Agent-centric accent colors based on theme
  const accent = colors.cyan; // Cyan/teal for primary accent
  const agent = colors.blue;
  const human = colors.green;
  const approval = colors.yellow;
  const reasoning = colors.magenta;
  const mission = colors.cyan;
  const timeline = colors.brightMagenta;

  return {
    // Core backgrounds
    "--bg-void": bgVoid,
    "--bg-deep": bgDeep,
    "--bg-primary": bgPrimary,
    "--bg-secondary": bgSecondary,
    "--bg-tertiary": bgTertiary,
    "--bg-surface": bgSurface,
    "--bg-elevated": bgElevated,
    "--bg-canvas": mix(colors.background, "#000000", 0.6),
    "--bg-overlay": withAlpha(mix(colors.background, "#000000", 0.4), 0.85),

    // Text colors
    "--text-primary": colors.foreground,
    "--text-secondary": mix(colors.foreground, colors.background, 0.28),
    "--text-muted": withAlpha(colors.foreground, 0.55),
    "--text-disabled": withAlpha(colors.foreground, 0.35),

    // Primary accent
    "--accent": accent,
    "--accent-hover": mix(accent, "#ffffff", 0.2),
    "--accent-soft": withAlpha(accent, 0.12),
    "--accent-dim": withAlpha(accent, 0.06),

    // Agent lane colors
    "--agent": agent,
    "--agent-soft": withAlpha(agent, 0.14),
    "--agent-glow": withAlpha(agent, 0.25),
    "--human": human,
    "--human-soft": withAlpha(human, 0.14),
    "--human-glow": withAlpha(human, 0.25),
    "--approval": approval,
    "--approval-soft": withAlpha(approval, 0.14),
    "--approval-glow": withAlpha(approval, 0.3),
    "--reasoning": reasoning,
    "--reasoning-soft": withAlpha(reasoning, 0.14),
    "--reasoning-glow": withAlpha(reasoning, 0.25),
    "--mission": mission,
    "--mission-soft": withAlpha(mission, 0.14),
    "--mission-glow": withAlpha(mission, 0.25),
    "--timeline": timeline,
    "--timeline-soft": withAlpha(timeline, 0.14),

    // Status colors
    "--success": colors.green,
    "--success-soft": withAlpha(colors.green, 0.12),
    "--warning": colors.yellow,
    "--warning-soft": withAlpha(colors.yellow, 0.12),
    "--danger": colors.red,
    "--danger-soft": withAlpha(colors.red, 0.12),
    "--info": colors.blue,
    "--info-soft": withAlpha(colors.blue, 0.12),

    // Risk levels
    "--risk-low": withAlpha(colors.green, 0.1),
    "--risk-medium": withAlpha(colors.yellow, 0.12),
    "--risk-high": withAlpha(colors.red, 0.12),
    "--risk-critical": withAlpha(colors.red, 0.18),

    // Borders
    "--border": withAlpha(colors.white, 0.06),
    "--border-strong": withAlpha(colors.white, 0.1),
    "--border-focus": withAlpha(accent, 0.4),
    "--glass-border": withAlpha(colors.white, 0.05),

    // Shadows
    "--shadow-sm": `0 2px 8px ${withAlpha(colors.black, 0.3)}`,
    "--shadow-md": `0 4px 16px ${withAlpha(colors.black, 0.4)}`,
    "--shadow-lg": `0 8px 32px ${withAlpha(colors.black, 0.5)}`,
    "--shadow-xl": `0 16px 48px ${withAlpha(colors.black, 0.6)}`,
    "--shadow-glow-sm": `0 0 20px ${withAlpha(agent, 0.3)}`,
    "--shadow-glow-md": `0 0 40px ${withAlpha(agent, 0.3)}`,

    // Blur
    "--blur-sm": "8px",
    "--blur-md": "16px",
    "--blur-lg": "24px",
    "--blur-xl": "32px",
    "--panel-blur": "20px",

    // Legacy compatibility
    "--shadow-color": withAlpha(colors.black, 0.45),
  };
}

export function applyAppShellTheme(theme: AppShellTheme): void {
  if (typeof document === "undefined") {
    return;
  }

  const root = document.documentElement;
  for (const [name, value] of Object.entries(theme)) {
    root.style.setProperty(name, value);
  }
}
