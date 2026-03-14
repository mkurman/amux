import type { TerminalNotification } from "../../lib/types";
import { formatTime } from "./shared";

export function NotificationList({
    notifications,
    markRead,
    onSelectNotification,
    onApproveNotification,
    onDenyNotification,
}: {
    notifications: TerminalNotification[];
    markRead: (id: string) => void;
    onSelectNotification?: (notification: TerminalNotification) => void;
    onApproveNotification?: (notification: TerminalNotification) => void;
    onDenyNotification?: (notification: TerminalNotification) => void;
}) {
    return (
        <div style={{ flex: 1, overflow: "auto", padding: "4px 0" }}>
            {notifications.length === 0 ? (
                <div
                    style={{
                        padding: 32,
                        textAlign: "center",
                        color: "var(--text-secondary)",
                        fontSize: 12,
                    }}
                >
                    No notifications
                </div>
            ) : (
                notifications.map((notification) => (
                    <div
                        key={notification.id}
                        onClick={() => {
                            markRead(notification.id);
                            onSelectNotification?.(notification);
                        }}
                        style={{
                            padding: "12px 16px",
                            cursor: "pointer",
                            borderBottom: "1px solid rgba(255,255,255,0.03)",
                            opacity: notification.isRead ? 0.5 : 1,
                            margin: "0 10px 8px",
                            borderRadius: 0,
                            border: notification.source === "approval"
                                ? "1px solid var(--approval-border)"
                                : "1px solid rgba(255,255,255,0.05)",
                            background: notification.source === "approval"
                                ? "var(--approval-soft)"
                                : "rgba(255,255,255,0.02)",
                        }}
                        onMouseEnter={(event) => {
                            event.currentTarget.style.background = "rgba(255,255,255,0.04)";
                        }}
                        onMouseLeave={(event) => {
                            event.currentTarget.style.background = "rgba(255,255,255,0.02)";
                        }}
                    >
                        <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 2 }}>
                            {!notification.isRead ? (
                                <div
                                    style={{
                                        width: 6,
                                        height: 6,
                                        borderRadius: "50%",
                                        background: "var(--accent)",
                                        flexShrink: 0,
                                    }}
                                />
                            ) : null}
                            <span style={{ fontSize: 12, fontWeight: notification.isRead ? 400 : 600 }}>
                                {notification.title}
                            </span>
                            <span style={{ marginLeft: "auto", fontSize: 10, color: "var(--text-secondary)" }}>
                                {formatTime(notification.timestamp)}
                            </span>
                        </div>
                        {notification.body ? (
                            <div
                                style={{
                                    fontSize: 11,
                                    color: "var(--text-secondary)",
                                    marginLeft: notification.isRead ? 0 : 12,
                                    whiteSpace: "pre-wrap",
                                    wordBreak: "break-word",
                                }}
                            >
                                {notification.body}
                            </div>
                        ) : null}
                        {notification.progress !== null ? (
                            <div
                                style={{
                                    marginTop: 4,
                                    marginLeft: notification.isRead ? 0 : 12,
                                    height: 3,
                                    borderRadius: 0,
                                    background: "var(--bg-surface)",
                                    overflow: "hidden",
                                }}
                            >
                                <div
                                    style={{
                                        width: `${notification.progress}%`,
                                        height: "100%",
                                        background: "var(--accent)",
                                        borderRadius: 0,
                                        transition: "width 0.3s ease",
                                    }}
                                />
                            </div>
                        ) : null}
                        {notification.source === "approval" ? (
                            <div style={{ marginTop: 10, display: "flex", gap: 8, marginLeft: notification.isRead ? 0 : 12 }}>
                                <button
                                    type="button"
                                    onClick={(event) => {
                                        event.stopPropagation();
                                        markRead(notification.id);
                                        onApproveNotification?.(notification);
                                    }}
                                    style={{
                                        border: "1px solid rgba(74, 222, 128, 0.36)",
                                        background: "rgba(74, 222, 128, 0.16)",
                                        color: "var(--success)",
                                        fontSize: 11,
                                        fontWeight: 700,
                                        padding: "5px 10px",
                                        cursor: "pointer",
                                    }}
                                >
                                    Allow (y)
                                </button>
                                <button
                                    type="button"
                                    onClick={(event) => {
                                        event.stopPropagation();
                                        markRead(notification.id);
                                        onDenyNotification?.(notification);
                                    }}
                                    style={{
                                        border: "1px solid rgba(248, 113, 113, 0.36)",
                                        background: "rgba(248, 113, 113, 0.14)",
                                        color: "var(--danger)",
                                        fontSize: 11,
                                        fontWeight: 700,
                                        padding: "5px 10px",
                                        cursor: "pointer",
                                    }}
                                >
                                    Deny (n)
                                </button>
                            </div>
                        ) : null}
                    </div>
                ))
            )}
        </div>
    );
}
