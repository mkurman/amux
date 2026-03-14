export type IconId =
  | "terminal"
  | "canvas"
  | "code"
  | "api"
  | "ops"
  | "db"
  | "ssh"
  | "web"
  | "ai"
  | "node"
  | "python"
  | "shell";

type IconDefinition = {
  id: IconId;
  label: string;
  glyph: string;
};

const ICONS: Record<IconId, IconDefinition> = {
  terminal: { id: "terminal", label: "Terminal", glyph: ">_" },
  canvas: { id: "canvas", label: "Canvas", glyph: "[]" },
  code: { id: "code", label: "Code", glyph: "{}" },
  api: { id: "api", label: "API", glyph: "<>" },
  ops: { id: "ops", label: "Ops", glyph: "##" },
  db: { id: "db", label: "Database", glyph: "DB" },
  ssh: { id: "ssh", label: "SSH", glyph: "SH" },
  web: { id: "web", label: "Web", glyph: "WB" },
  ai: { id: "ai", label: "AI", glyph: "AI" },
  node: { id: "node", label: "Node", glyph: "JS" },
  python: { id: "python", label: "Python", glyph: "PY" },
  shell: { id: "shell", label: "Shell", glyph: "$" },
};

const ICON_ALIASES: Record<string, IconId> = {
  terminal: "terminal",
  term: "terminal",
  shell: "shell",
  canvas: "canvas",
  code: "code",
  api: "api",
  ops: "ops",
  db: "db",
  ssh: "ssh",
  web: "web",
  ai: "ai",
  node: "node",
  python: "python",
};

export const WORKSPACE_ICON_IDS: IconId[] = [
  "terminal",
  "code",
  "api",
  "ops",
  "db",
  "ssh",
  "web",
  "ai",
];

export const SURFACE_ICON_IDS: IconId[] = [
  "terminal",
  "canvas",
  "code",
  "api",
  "ops",
  "db",
  "ssh",
  "web",
];

export const PANE_ICON_IDS: IconId[] = [
  "terminal",
  "shell",
  "code",
  "api",
  "ops",
  "db",
  "ssh",
  "web",
  "node",
  "python",
  "ai",
];

export function normalizeIconId(value: string | null | undefined): IconId {
  const candidate = typeof value === "string" ? value.trim().toLowerCase() : "";
  if (!candidate) return "terminal";
  return ICON_ALIASES[candidate] ?? "terminal";
}

export function iconLabel(value: string | null | undefined): string {
  return ICONS[normalizeIconId(value)].label;
}

export function iconGlyph(value: string | null | undefined): string {
  return ICONS[normalizeIconId(value)].glyph;
}

export function iconChoices(ids: IconId[]): IconDefinition[] {
  return ids.map((id) => ICONS[id]);
}
