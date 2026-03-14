import { useState } from "react";
import type { Workspace } from "../../lib/types";
import { ICON_CHOICES } from "./shared";

export function WorkspaceItem({
    workspace,
    index,
    isActive,
    unreadCount,
    onSelect,
    onClose,
    onRename,
    onSetIcon,
    children,
}: {
    workspace: Workspace;
    index: number;
    isActive: boolean;
    unreadCount: number;
    onSelect: () => void;
    onClose: () => void;
    onRename: (name: string) => void;
    onSetIcon: (icon: string) => void;
    children?: React.ReactNode;
}) {
    const [editing, setEditing] = useState(false);
    const [draftName, setDraftName] = useState(workspace.name);
    const [draftIcon, setDraftIcon] = useState(workspace.icon);

    const commit = () => {
        onRename(draftName.trim() || workspace.name);
        onSetIcon(draftIcon.trim() || workspace.icon);
        setEditing(false);
    };

    return (
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-1)" }}>
            <div
                onClick={onSelect}
                style={{
                display: "flex",
                alignItems: "center",
                gap: "var(--space-3)",
                padding: "var(--space-3)",
                cursor: "pointer",
                background: isActive ? "var(--bg-tertiary)" : "transparent",
                border: "1px solid",
                borderColor: isActive ? "var(--glass-border)" : "transparent",
                borderLeft: `2px solid ${isActive ? workspace.accentColor : "transparent"}`,
                transition: "background var(--transition-fast)",
            }}
                onMouseEnter={(e) => {
                    if (!isActive) {
                        e.currentTarget.style.background = "var(--bg-secondary)";
                    }
                }}
                onMouseLeave={(e) => {
                    if (!isActive) {
                        e.currentTarget.style.background = "transparent";
                    }
                }}
            >
            <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: "var(--space-1)", flexShrink: 0 }}>
                <div style={iconBadgeStyle}>{workspace.icon}</div>

                <div
                    style={{
                        width: 8,
                        height: 8,
                        borderRadius: "50%",
                        background: workspace.accentColor,
                        border: `1px solid ${workspace.accentColor}`,
                    }}
                />
            </div>

            <div style={{ flex: 1, overflow: "hidden", minWidth: 0 }}>
                {editing ? (
                    <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }} onClick={(event) => event.stopPropagation()}>
                        <input
                            type="text"
                            value={draftName}
                            onChange={(event) => setDraftName(event.target.value)}
                            onKeyDown={(event) => {
                                if (event.key === "Enter") commit();
                                if (event.key === "Escape") {
                                    setDraftName(workspace.name);
                                    setDraftIcon(workspace.icon);
                                    setEditing(false);
                                }
                            }}
                            autoFocus
                            style={editInputStyle}
                        />
                        <select value={draftIcon} onChange={(event) => setDraftIcon(event.target.value)} style={editInputStyle}>
                            {ICON_CHOICES.map((icon) => (
                                <option key={icon} value={icon}>
                                    {icon}
                                </option>
                            ))}
                        </select>

                        <div style={{ display: "flex", justifyContent: "flex-end", gap: "var(--space-2)" }}>
                            <button type="button" style={secondaryEditButtonStyle} onClick={() => {
                                setDraftName(workspace.name);
                                setDraftIcon(workspace.icon);
                                setEditing(false);
                            }}>
                                Cancel
                            </button>
                            <button type="button" style={primaryEditButtonStyle} onClick={commit}>
                                Save
                            </button>
                        </div>
                    </div>
                ) : (
                    <>
                        <div style={{ fontSize: "var(--text-sm)", color: isActive ? "var(--text-primary)" : "var(--text-secondary)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", fontWeight: isActive ? 600 : 400 }}>
                            {workspace.name}
                        </div>

                        {workspace.gitBranch ? (
                            <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", marginTop: 2 }}>
                                <span style={{ opacity: 0.6 }}>⎇</span> {workspace.gitBranch}
                                {workspace.gitDirty ? (
                                    <span style={{ color: "var(--warning)", marginLeft: 4 }}>●</span>
                                ) : null}
                            </div>
                        ) : null}

                        <div style={{ display: "flex", gap: "var(--space-1)", flexWrap: "wrap", marginTop: "var(--space-2)" }}>
                            {workspace.cwd ? (
                                <span className="amux-chip amux-code" style={{ maxWidth: 160, overflow: "hidden", textOverflow: "ellipsis", fontSize: "var(--text-xs)" }}>
                                    {workspace.cwd}
                                </span>
                            ) : null}

                            {workspace.listeningPorts.length > 0 ? (
                                <span className="amux-chip" style={{ fontSize: "var(--text-xs)" }}>
                                    :{workspace.listeningPorts.join(",")}
                                </span>
                            ) : null}
                        </div>
                    </>
                )}
            </div>

            {index <= 9 ? (
                <span style={indexBadgeStyle}>
                    {index}
                </span>
            ) : null}

            {unreadCount > 0 ? (
                <div style={unreadBadgeStyle}>
                    {unreadCount > 99 ? "99+" : unreadCount}
                </div>
            ) : null}

            <div style={{ display: "flex", gap: "var(--space-1)", alignItems: "center" }}>
                <button
                    onClick={(e) => {
                        e.stopPropagation();
                        setEditing((current) => !current);
                    }}
                    style={iconActionButtonStyle}
                    onMouseEnter={(e) => (e.currentTarget.style.opacity = "1")}
                    onMouseLeave={(e) => (e.currentTarget.style.opacity = "0.6")}
                    title="Rename workspace"
                >
                    ✎
                </button>

                <button
                    onClick={(e) => {
                        e.stopPropagation();
                        onClose();
                    }}
                    style={iconActionButtonStyle}
                    onMouseEnter={(e) => (e.currentTarget.style.opacity = "1")}
                    onMouseLeave={(e) => (e.currentTarget.style.opacity = "0.6")}
                    title="Close workspace"
                >
                    ×
                </button>
            </div>
            </div>
            {children}
        </div>
    );
}

const iconBadgeStyle: React.CSSProperties = {
    minWidth: 32,
    height: 28,
    borderRadius: "var(--radius-sm)",
    background: "var(--bg-surface)",
    border: "1px solid var(--glass-border)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    padding: "0 var(--space-2)",
    fontSize: "var(--text-xs)",
    textTransform: "uppercase",
    color: "var(--text-secondary)",
    fontWeight: 600,
};

const editInputStyle: React.CSSProperties = {
    width: "100%",
    background: "var(--bg-surface)",
    border: "1px solid var(--glass-border)",
    color: "var(--text-primary)",
    borderRadius: "var(--radius-sm)",
    padding: "var(--space-1) var(--space-2)",
    fontSize: "var(--text-sm)",
    outline: "none",
};

const secondaryEditButtonStyle: React.CSSProperties = {
    background: "var(--bg-surface)",
    border: "1px solid var(--glass-border)",
    borderRadius: "var(--radius-sm)",
    color: "var(--text-muted)",
    cursor: "pointer",
    fontSize: "var(--text-xs)",
    padding: "var(--space-1) var(--space-2)",
};

const primaryEditButtonStyle: React.CSSProperties = {
    background: "var(--accent-soft)",
    border: "1px solid var(--accent-soft)",
    borderRadius: "var(--radius-sm)",
    color: "var(--accent)",
    cursor: "pointer",
    fontSize: "var(--text-xs)",
    padding: "var(--space-1) var(--space-2)",
};

const indexBadgeStyle: React.CSSProperties = {
    fontSize: "var(--text-xs)",
    color: "var(--text-muted)",
    opacity: 0.6,
    background: "var(--bg-surface)",
    border: "1px solid var(--glass-border)",
    borderRadius: "var(--radius-full)",
    padding: "2px 6px",
};

const unreadBadgeStyle: React.CSSProperties = {
    background: "var(--accent)",
    color: "var(--bg-primary)",
    borderRadius: "var(--radius-full)",
    padding: "0 var(--space-1)",
    fontSize: "var(--text-xs)",
    fontWeight: 700,
    minWidth: 16,
    textAlign: "center",
    lineHeight: "18px",
};

const iconActionButtonStyle: React.CSSProperties = {
    background: "transparent",
    border: "none",
    color: "var(--text-muted)",
    cursor: "pointer",
    fontSize: "var(--text-sm)",
    padding: "var(--space-1)",
    opacity: 0.6,
    transition: "opacity var(--transition-fast)",
};
