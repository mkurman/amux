import type { CSSProperties } from "react";
import type { useWorkspaceStore } from "../../lib/workspaceStore";
import { SURFACE_ICON_IDS } from "../../lib/iconRegistry";

export const SURFACE_ICONS = SURFACE_ICON_IDS;

export type SurfaceRecord = ReturnType<typeof useWorkspaceStore.getState>["workspaces"][number]["surfaces"][number];

export const dividerStyle: CSSProperties = {
    width: 1,
    height: 20,
    background: "var(--border)",
    flexShrink: 0,
};

export const actionButtonBaseStyle: CSSProperties = {
    background: "transparent",
    border: "1px solid transparent",
    color: "var(--text-muted)",
    cursor: "pointer",
    fontSize: "var(--text-xs)",
    padding: "0 var(--space-2)",
    height: 24,
    minWidth: 28,
    borderRadius: "var(--radius-sm)",
    transition: "all var(--transition-fast)",
};
