import { useEffect, useRef, useState } from "react";
import { SURFACE_ICONS, type SurfaceRecord } from "./shared";
import { iconChoices, iconGlyph, iconLabel, normalizeIconId } from "../../lib/iconRegistry";

export function SurfaceTabItem({
    surface,
    isActive,
    accentColor,
    approvalCount,
    paneCount,
    onSelect,
    onClose,
    onRename,
    onSetIcon,
}: {
    surface: SurfaceRecord;
    isActive: boolean;
    accentColor: string;
    approvalCount: number;
    paneCount: number;
    onSelect: () => void;
    onClose: () => void;
    onRename: (name: string) => void;
    onSetIcon: (icon: string) => void;
}) {
    const rootRef = useRef<HTMLDivElement | null>(null);
    const [editing, setEditing] = useState(false);
    const [draftName, setDraftName] = useState(surface.name);
    const [iconMenuOpen, setIconMenuOpen] = useState(false);
    const commitTimeoutRef = useRef<number | null>(null);

    const cancelScheduledCommit = () => {
        if (commitTimeoutRef.current !== null) {
            window.clearTimeout(commitTimeoutRef.current);
            commitTimeoutRef.current = null;
        }
    };

    const commit = () => {
        cancelScheduledCommit();
        onRename(draftName.trim() || surface.name);
        setEditing(false);
    };

    const scheduleCommit = () => {
        commitTimeoutRef.current = window.setTimeout(() => {
            commit();
        }, 150);
    };

    useEffect(() => () => cancelScheduledCommit(), []);

    useEffect(() => {
        if (!iconMenuOpen) return;
        const onPointerDown = (event: MouseEvent) => {
            if (rootRef.current?.contains(event.target as Node)) {
                return;
            }
            setIconMenuOpen(false);
        };
        window.addEventListener("mousedown", onPointerDown);
        return () => window.removeEventListener("mousedown", onPointerDown);
    }, [iconMenuOpen]);

    return (
        <div
            ref={rootRef}
            onClick={onSelect}
            onDoubleClick={() => setEditing(true)}
            style={{
                position: "relative",
                display: "flex",
                alignItems: "center",
                gap: "var(--space-2)",
                padding: "0 var(--space-3)",
                height: 28,
                fontSize: "var(--text-xs)",
                cursor: "pointer",
                background: isActive ? "var(--bg-tertiary)" : "transparent",
                color: isActive ? "var(--text-primary)" : "var(--text-muted)",
                border: "1px solid",
                borderColor: isActive ? accentColor : "transparent",
                borderRadius: "var(--radius-md)",
                whiteSpace: "nowrap",
                transition: "all var(--transition-fast)",
            }}
        >
            <button
                type="button"
                onClick={(event) => {
                    event.stopPropagation();
                    setIconMenuOpen((value) => !value);
                }}
                title={`Icon: ${iconLabel(surface.icon)}`}
                style={{
                    border: "none",
                    background: "transparent",
                    color: "inherit",
                    cursor: "pointer",
                    fontSize: "var(--text-sm)",
                    padding: 0,
                    lineHeight: 1,
                }}
            >
                {iconGlyph(surface.icon)}
            </button>

            {editing ? (
                <div style={{ display: "flex", gap: "var(--space-1)" }} onClick={(event) => event.stopPropagation()}>
                    <input
                        type="text"
                        value={draftName}
                        onChange={(event) => setDraftName(event.target.value)}
                        onBlur={scheduleCommit}
                        onKeyDown={(event) => {
                            if (event.key === "Enter") commit();
                            if (event.key === "Escape") {
                                setDraftName(surface.name);
                                setEditing(false);
                            }
                        }}
                        autoFocus
                        style={{
                            background: "var(--bg-surface)",
                            border: "1px solid var(--glass-border)",
                            color: "var(--text-primary)",
                            borderRadius: "var(--radius-sm)",
                            padding: "2px 6px",
                            fontSize: "var(--text-xs)",
                            outline: "none",
                            width: 100,
                        }}
                    />
                </div>
            ) : (
                <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
                    <span style={{ fontWeight: isActive ? 600 : 400 }}>{surface.name}</span>
                    <span style={{ opacity: 0.6 }}>
                        {paneCount}
                        {approvalCount > 0 ? <span style={{ color: "var(--approval)", marginLeft: "var(--space-1)" }}>· {approvalCount}</span> : null}
                    </span>
                </div>
            )}

            <button
                onClick={(event) => {
                    event.stopPropagation();
                    if (editing) {
                        commit();
                    } else {
                        setDraftName(surface.name);
                        setEditing(true);
                    }
                }}
                style={{
                    background: "transparent",
                    border: "none",
                    color: "var(--text-muted)",
                    cursor: "pointer",
                    fontSize: "var(--text-xs)",
                    padding: "0 var(--space-1)",
                    opacity: 0,
                    transition: "opacity var(--transition-fast)",
                }}
                onMouseEnter={(event) => {
                    event.currentTarget.style.opacity = "1";
                }}
            >
                ✎
            </button>

            <button
                onClick={(event) => {
                    event.stopPropagation();
                    onClose();
                }}
                style={{
                    background: "transparent",
                    border: "none",
                    color: "var(--text-muted)",
                    cursor: "pointer",
                    fontSize: "var(--text-sm)",
                    padding: "0 var(--space-1)",
                    opacity: 0,
                    transition: "opacity var(--transition-fast)",
                }}
                onMouseEnter={(event) => {
                    event.currentTarget.style.opacity = "1";
                }}
            >
                ×
            </button>

            {iconMenuOpen ? (
                <div
                    onClick={(event) => event.stopPropagation()}
                    style={{
                        position: "absolute",
                        top: 30,
                        left: 0,
                        minWidth: 160,
                        border: "1px solid var(--glass-border)",
                        borderRadius: "var(--radius-md)",
                        background: "var(--bg-primary)",
                        boxShadow: "var(--shadow-sm)",
                        zIndex: 70,
                        display: "grid",
                        gap: 2,
                        padding: 4,
                    }}
                >
                    {iconChoices(SURFACE_ICONS).map((icon) => (
                        <button
                            key={icon.id}
                            type="button"
                            onClick={() => {
                                onSetIcon(normalizeIconId(icon.id));
                                setIconMenuOpen(false);
                            }}
                            style={{
                                border: "none",
                                background: "transparent",
                                color: "var(--text-secondary)",
                                cursor: "pointer",
                                textAlign: "left",
                                fontSize: "var(--text-xs)",
                                borderRadius: "var(--radius-sm)",
                                padding: "6px 8px",
                                display: "flex",
                                alignItems: "center",
                                gap: 8,
                            }}
                        >
                            <span style={{ minWidth: 24, textAlign: "center", fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace" }}>{icon.glyph}</span>
                            <span>{icon.label}</span>
                        </button>
                    ))}
                </div>
            ) : null}
        </div>
    );
}
