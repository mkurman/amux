import { useMemo, useState } from "react";
import type React from "react";
import ReactMarkdown from "react-markdown";
import type { Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import type { AgentMessage, AgentThread } from "../../lib/agentStore";
import { inputStyle } from "./shared";

export function ChatView({
    messages,
    input,
    setInput,
    inputRef,
    onKeyDown,
    agentSettings,
    isStreamingResponse,
    activeThread,
    messagesEndRef,
    onSendMessage,
    onStopStreaming,
}: {
    messages: AgentMessage[];
    input: string;
    setInput: (v: string) => void;
    inputRef: React.RefObject<HTMLTextAreaElement | null>;
    onKeyDown: (e: React.KeyboardEvent) => void;
    agentSettings: { enabled: boolean; chatFontFamily: string };
    isStreamingResponse: boolean;
    activeThread: AgentThread | undefined;
    messagesEndRef: React.RefObject<HTMLDivElement | null>;
    onSendMessage: (text: string) => void;
    onStopStreaming: () => void;
}) {
    const [searchQuery, setSearchQuery] = useState("");

    const handleSendClick = () => {
        const text = input.trim();
        if (!text) return;
        onSendMessage(text);
        setInput("");
    };

    const displayItems = useMemo(() => {
        type ToolGroup = {
            key: string;
            toolCallId: string;
            toolName: string;
            toolArguments: string;
            status: "requested" | "executing" | "done" | "error";
            resultContent: string;
            createdAt: number;
        };

        const items: Array<{ type: "message"; message: AgentMessage } | { type: "tool"; group: ToolGroup }> = [];
        const groups = new Map<string, ToolGroup>();

        for (const message of messages) {
            if (message.role !== "tool") {
                items.push({ type: "message", message });
                continue;
            }

            const groupKey = message.toolCallId || message.id;
            const existing = groups.get(groupKey);

            if (!existing) {
                const initialGroup: ToolGroup = {
                    key: groupKey,
                    toolCallId: message.toolCallId || message.id,
                    toolName: message.toolName || "tool",
                    toolArguments: message.toolArguments || "",
                    status: message.toolStatus || (message.content ? "done" : "requested"),
                    resultContent: message.content || "",
                    createdAt: message.createdAt,
                };
                groups.set(groupKey, initialGroup);
                items.push({ type: "tool", group: initialGroup });
                continue;
            }

            if (message.toolName) existing.toolName = message.toolName;
            if (message.toolArguments) existing.toolArguments = message.toolArguments;
            if (message.toolStatus) {
                existing.status = message.toolStatus;
            } else if (message.content) {
                existing.status = "done";
            }
            if (message.content) existing.resultContent = message.content;
            existing.createdAt = Math.min(existing.createdAt, message.createdAt);
        }

        return items;
    }, [messages]);

    const filteredDisplayItems = useMemo(() => {
        const normalizedQuery = searchQuery.trim().toLowerCase();

        return displayItems.filter((item) => {
            if (!normalizedQuery) {
                return true;
            }

            if (item.type === "message") {
                const message = item.message;
                return [
                    message.role,
                    message.content,
                    message.reasoning ?? "",
                    message.provider ?? "",
                    message.model ?? "",
                ].join(" ").toLowerCase().includes(normalizedQuery);
            }

            return [
                item.group.toolName,
                item.group.toolArguments,
                item.group.resultContent,
                item.group.status,
            ].join(" ").toLowerCase().includes(normalizedQuery);
        });
    }, [displayItems, searchQuery]);

    const sessionUsageSummary = useMemo(() => {
        let totalCost = 0;
        let hasCost = false;
        let tpsSum = 0;
        let tpsCount = 0;

        for (const message of messages) {
            if (message.role !== "assistant") continue;
            if (typeof message.cost === "number" && Number.isFinite(message.cost)) {
                totalCost += message.cost;
                hasCost = true;
            }
            if (typeof message.tps === "number" && Number.isFinite(message.tps) && message.tps > 0) {
                tpsSum += message.tps;
                tpsCount += 1;
            }
        }

        return {
            hasCost,
            totalCost,
            avgTps: tpsCount > 0 ? (tpsSum / tpsCount) : undefined,
        };
    }, [messages]);

    return (
        <>
            <div
                style={{
                    flex: 1,
                    overflow: "auto",
                    padding: "8px 8px 8px 16px",
                    display: "flex",
                    flexDirection: "column",
                    gap: "var(--space-3)",
                }}
            >
                <div style={{ display: "flex", gap: "var(--space-3)", flexWrap: "wrap", alignItems: "center" }}>
                    <input
                        type="text"
                        value={searchQuery}
                        onChange={(event) => setSearchQuery(event.target.value)}
                        placeholder="Search messages and tool output..."
                        style={{ ...inputStyle, minWidth: 220 }}
                    />
                </div>

                {filteredDisplayItems.length === 0 && (
                    <div className="amux-empty-state">
                        <div className="amux-empty-state__icon">✨</div>
                        <div className="amux-empty-state__title">{messages.length === 0 ? "Start a conversation" : "No chat items match filters"}</div>
                        <div className="amux-empty-state__description">{messages.length === 0 ? "Send a message to begin collaborating with the agent" : "Try a different search term."}</div>
                    </div>
                )}

                {filteredDisplayItems.map((item) => {
                    if (item.type === "tool") {
                        return <ToolEventRow key={`tool_${item.group.key}`} group={item.group} />;
                    }

                    const msg = item.message;
                    return (
                        <MessageBubble
                            key={msg.id}
                            message={msg}
                            onCopy={() => {
                                try { navigator.clipboard.writeText(msg.content); } catch { /* silent */ }
                            }}
                            onRerun={msg.role === "user" ? () => {
                                onSendMessage(msg.content);
                            } : undefined}
                            onRegenerate={msg.role === "assistant" ? () => {
                                const idx = messages.findIndex((m) => m.id === msg.id);
                                if (idx > 0) {
                                    const prevUserMsg = messages.slice(0, idx).reverse().find((m) => m.role === "user");
                                    if (prevUserMsg) {
                                        onSendMessage(prevUserMsg.content);
                                    }
                                }
                            } : undefined}
                        />
                    );
                })}
                <div ref={messagesEndRef} />
            </div>

            {activeThread && activeThread.totalTokens > 0 && (
                <div
                    style={{
                        padding: "var(--space-2) var(--space-3)",
                        fontSize: "var(--text-xs)",
                        color: "var(--text-muted)",
                        borderTop: "1px solid var(--border)",
                        display: "flex",
                        gap: "var(--space-3)",
                    }}
                >
                    <span>In: {activeThread.totalInputTokens.toLocaleString()}</span>
                    <span>Out: {activeThread.totalOutputTokens.toLocaleString()}</span>
                    <span>Total: {activeThread.totalTokens.toLocaleString()}</span>
                    {sessionUsageSummary.hasCost && (
                        <span>Cost: ${sessionUsageSummary.totalCost.toFixed(6)}</span>
                    )}
                    {typeof sessionUsageSummary.avgTps === "number" && (
                        <span>Avg TPS: {sessionUsageSummary.avgTps.toFixed(1)} tok/s</span>
                    )}
                    {activeThread.compactionCount > 0 && (
                        <span>Compacted: {activeThread.compactionCount}×</span>
                    )}
                </div>
            )}

            <div
                style={{
                    padding: "var(--space-3)",
                    borderTop: "1px solid var(--border)",
                    flexShrink: 0,
                    display: "flex",
                    flexDirection: "column",
                    background: "var(--bg-tertiary)",
                    userSelect: "auto",
                }}
            >
                <div
                    style={{
                        display: "grid",
                        gridTemplateColumns: "auto 1fr",
                        alignItems: "start",
                        gap: "var(--space-2)",
                        border: "1px solid rgba(94, 231, 223, 0.3)",
                        background: "var(--bg-tertiary)",
                        borderRadius: "var(--radius-md)",
                        padding: "8px 10px",
                    }}
                >
                    <span
                        style={{
                            color: "#5ee7df",
                            fontFamily: "var(--font-mono)",
                            fontSize: "var(--text-sm)",
                            lineHeight: "24px",
                            userSelect: "auto",
                        }}
                    >
                        &gt;
                    </span>
                    <textarea
                        ref={inputRef}
                        value={input}
                        onChange={(e) => setInput(e.target.value)}
                        onKeyDown={onKeyDown}
                        rows={3}
                        placeholder={
                            agentSettings.enabled
                                ? "Type a message... (Enter to send, Shift+Enter for newline)"
                                : "Agent disabled — enable in Settings > Agent"
                        }
                        disabled={!agentSettings.enabled}
                        style={{
                            width: "100%",
                            resize: "none",
                            background: "transparent",
                            border: "none",
                            color: "var(--text-primary)",
                            fontSize: "var(--text-sm)",
                            padding: "4px 0",
                            fontFamily: agentSettings.chatFontFamily,
                            outline: "none",
                            opacity: agentSettings.enabled ? 1 : 0.5,
                            minHeight: 72,
                        }}
                    />
                </div>

                <div style={{ marginTop: "var(--space-2)", display: "flex", justifyContent: "space-between", alignItems: "center", gap: "var(--space-2)" }}>
                    <span style={{ fontSize: 11, color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
                        Enter send, Shift+Enter newline
                    </span>
                    <div style={{ display: "flex", gap: "var(--space-2)" }}>
                        {isStreamingResponse && (
                            <button
                                type="button"
                                onClick={onStopStreaming}
                                style={{
                                    border: "1px solid rgba(255, 118, 117, 0.45)",
                                    background: "rgba(255, 118, 117, 0.15)",
                                    color: "#ff7675",
                                    borderRadius: "var(--radius-sm)",
                                    padding: "6px 10px",
                                    fontSize: 12,
                                    fontWeight: 600,
                                    cursor: "pointer",
                                }}
                            >
                                Stop
                            </button>
                        )}
                        <button
                            type="button"
                            onClick={handleSendClick}
                            disabled={!agentSettings.enabled || !input.trim()}
                            style={{
                                border: "1px solid var(--accent)",
                                background: "rgba(94, 231, 223, 0.16)",
                                color: "var(--accent)",
                                borderRadius: "var(--radius-sm)",
                                padding: "6px 12px",
                                fontSize: 12,
                                fontWeight: 700,
                                cursor: !agentSettings.enabled || !input.trim() ? "not-allowed" : "pointer",
                                opacity: !agentSettings.enabled || !input.trim() ? 0.5 : 1,
                            }}
                        >
                            Send
                        </button>
                    </div>
                </div>
            </div>
        </>
    );
}

const markdownComponents: Components = {
    p: ({ children }) => (
        <p style={{ margin: "4px 0" }}>{children}</p>
    ),
    a: ({ href, children }) => (
        <a
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            style={{ color: "#5ee7df", textDecoration: "underline", textUnderlineOffset: 2 }}
        >
            {children}
        </a>
    ),
    pre: ({ children }) => (
        <pre
            style={{
                margin: "6px 0",
                padding: "10px 12px",
                background: "rgba(0, 0, 0, 0.35)",
                borderRadius: "var(--radius-md)",
                overflowX: "auto",
                fontSize: "var(--text-xs)",
                lineHeight: 1.5,
                border: "1px solid rgba(255,255,255,0.08)",
            }}
        >
            {children}
        </pre>
    ),
    code: ({ className, children }) => {
        const isBlock = className?.startsWith("language-");
        if (isBlock) {
            return (
                <code style={{ fontFamily: "var(--font-mono)", fontSize: "inherit" }}>
                    {children}
                </code>
            );
        }
        return (
            <code
                style={{
                    fontFamily: "var(--font-mono)",
                    background: "rgba(255, 255, 255, 0.08)",
                    padding: "1px 5px",
                    borderRadius: 3,
                    fontSize: "0.9em",
                }}
            >
                {children}
            </code>
        );
    },
    ul: ({ children }) => (
        <ul style={{ margin: "4px 0", paddingLeft: 20 }}>{children}</ul>
    ),
    ol: ({ children }) => (
        <ol style={{ margin: "4px 0", paddingLeft: 20 }}>{children}</ol>
    ),
    li: ({ children }) => (
        <li style={{ margin: "2px 0" }}>{children}</li>
    ),
    h1: ({ children }) => (
        <h4 style={{ margin: "8px 0 4px", fontSize: "1.1em", fontWeight: 700 }}>{children}</h4>
    ),
    h2: ({ children }) => (
        <h5 style={{ margin: "8px 0 4px", fontSize: "1.05em", fontWeight: 700 }}>{children}</h5>
    ),
    h3: ({ children }) => (
        <h6 style={{ margin: "6px 0 4px", fontSize: "1em", fontWeight: 600 }}>{children}</h6>
    ),
    h4: ({ children }) => (
        <h6 style={{ margin: "6px 0 4px", fontSize: "0.95em", fontWeight: 600 }}>{children}</h6>
    ),
    h5: ({ children }) => (
        <h6 style={{ margin: "4px 0 2px", fontSize: "0.9em", fontWeight: 600 }}>{children}</h6>
    ),
    h6: ({ children }) => (
        <h6 style={{ margin: "4px 0 2px", fontSize: "0.85em", fontWeight: 600 }}>{children}</h6>
    ),
    blockquote: ({ children }) => (
        <blockquote
            style={{
                margin: "6px 0",
                paddingLeft: 12,
                borderLeft: "3px solid rgba(94, 231, 223, 0.4)",
                color: "var(--text-secondary)",
                fontStyle: "italic",
            }}
        >
            {children}
        </blockquote>
    ),
    table: ({ children }) => (
        <div style={{ overflowX: "auto", margin: "6px 0" }}>
            <table
                style={{
                    width: "100%",
                    borderCollapse: "collapse",
                    fontSize: "var(--text-xs)",
                }}
            >
                {children}
            </table>
        </div>
    ),
    th: ({ children }) => (
        <th
            style={{
                textAlign: "left",
                padding: "4px 8px",
                borderBottom: "1px solid rgba(255,255,255,0.15)",
                fontWeight: 600,
            }}
        >
            {children}
        </th>
    ),
    td: ({ children }) => (
        <td
            style={{
                padding: "4px 8px",
                borderBottom: "1px solid rgba(255,255,255,0.06)",
            }}
        >
            {children}
        </td>
    ),
    hr: () => (
        <hr
            style={{
                border: "none",
                borderTop: "1px solid rgba(255,255,255,0.1)",
                margin: "8px 0",
            }}
        />
    ),
};

function MarkdownContent({ content }: { content: string }) {
    return (
        <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
            {content}
        </ReactMarkdown>
    );
}

function ToolEventRow({
    group,
}: {
    group: {
        key: string;
        toolCallId: string;
        toolName: string;
        toolArguments: string;
        status: "requested" | "executing" | "done" | "error";
        resultContent: string;
    };
}) {
    const [collapsed, setCollapsed] = useState(true);
    const statusLabel = group.status.toUpperCase();
    const shortId = (group.toolCallId || group.key).slice(-8);

    return (
        <div style={{ marginLeft: 24, fontFamily: "var(--font-mono)", whiteSpace: "pre-wrap", wordBreak: "break-word" }}>
            <button
                type="button"
                onClick={() => setCollapsed((prev) => !prev)}
                style={{
                    border: "none",
                    background: "transparent",
                    padding: 0,
                    color: "var(--text-primary)",
                    cursor: "pointer",
                    fontFamily: "var(--font-mono)",
                    fontSize: "var(--text-sm)",
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                }}
            >
                <span style={{ color: "#DE600A" }}>{collapsed ? "▸" : "▾"}</span>
                <span style={{ color: "#DE600A" }}>{"tool"}</span>
                <span>{group.toolName}</span>
                <span style={{ color: "#BA4400", fontSize: 11 }}>#{shortId}</span>
                <span style={{ color: "#BA4400", fontSize: 11 }}>{statusLabel}</span>
            </button>

            {!collapsed && (
                <div style={{ marginLeft: 20, marginTop: 6, display: "grid", gap: 6 }}>
                    {group.toolArguments && (
                        <div>
                            <div style={{ color: "var(--text-muted)", fontSize: 11 }}>args</div>
                            <pre style={{ margin: 0, fontFamily: "var(--font-mono)", fontSize: 11, color: "var(--text-primary)", whiteSpace: "pre-wrap" }}>
                                {(() => {
                                    try {
                                        return JSON.stringify(JSON.parse(group.toolArguments), null, 2);
                                    } catch {
                                        return group.toolArguments;
                                    }
                                })()}
                            </pre>
                        </div>
                    )}

                    {group.resultContent && (
                        <div>
                            <div style={{ color: "var(--text-muted)", fontSize: 11 }}>result</div>
                            <div style={{ fontSize: 12, lineHeight: 1.45 }}>{group.resultContent}</div>
                        </div>
                    )}
                </div>
            )}
        </div>
    );
}

function MessageBubble({
    message,
    onCopy,
    onRerun,
    onRegenerate,
}: {
    message: AgentMessage;
    onCopy?: () => void;
    onRerun?: () => void;
    onRegenerate?: () => void;
}) {
    const isUser = message.role === "user";
    const isSystem = message.role === "system";
    const isTool = message.role === "tool";
    const isAssistant = message.role === "assistant";
    const toolStatusLabel = message.toolStatus
        ? message.toolStatus.toUpperCase()
        : "DONE";
    const [hovered, setHovered] = useState(false);
    const [copied, setCopied] = useState(false);
    const displayContent = (() => {
        if (!isUser || typeof message.content !== "string") return message.content;
        if (!message.content.startsWith("[Gateway Context]")) return message.content;

        const marker = "User message:\n";
        const markerIndex = message.content.indexOf(marker);
        if (markerIndex < 0) return message.content;

        return message.content.slice(markerIndex + marker.length).trim();
    })();

    const handleCopy = () => {
        onCopy?.();
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
    };

    return (
        <div
            style={{
                display: "flex",
                justifyContent: isUser ? "flex-end" : "flex-start",
            }}
        >
            <div
                style={{
                    maxWidth: "85%",
                    position: "relative",
                    padding: "var(--space-3)",
                    borderRadius: "var(--radius-lg)",
                    fontSize: "var(--text-sm)",
                    lineHeight: 1.6,
                    background: isUser
                        ? "var(--bg-secondary)"
                        : isSystem || isTool
                            ? "var(--bg-secondary)"
                            : "transparent",
                    color: isUser ? "#b2fff8" : "var(--text-primary)",
                    border: "1px solid",
                    borderColor: isUser
                        ? "rgba(94, 231, 223, 0.28)"
                        : isSystem || isTool
                            ? "rgba(120, 168, 209, 0.22)"
                            : "transparent",
                    wordBreak: "break-word",
                    userSelect: "auto",
                    fontFamily: "var(--font-mono)",
                    paddingLeft: isAssistant ? 0 : "var(--space-3)",
                    paddingRight: isAssistant ? 0 : "var(--space-3)",
                }}
                onMouseEnter={() => setHovered(true)}
                onMouseLeave={() => setHovered(false)}
            >
                {isAssistant && (
                    <div style={{ color: "#5ee7df", opacity: 0.95, marginBottom: 4, fontSize: 12 }}>{"> assistant"}</div>
                )}

                {isAssistant && message.reasoning && (
                    <details style={{ marginTop: 8 }}>
                        <summary style={{ cursor: "pointer", fontSize: 11, color: "var(--text-muted)", userSelect: "auto" }}>
                            Reasoning
                        </summary>
                        <div style={{ marginTop: 6, fontSize: 12, color: "var(--text-secondary)", userSelect: "auto" }}>
                            <MarkdownContent content={message.reasoning} />
                        </div>
                    </details>
                )}

                {isTool && message.toolName ? (
                    <div style={{ display: "grid", gap: 6 }}>
                        <div style={{ display: "flex", justifyContent: "space-between", gap: 8, alignItems: "center" }}>
                            <span style={{ fontSize: "var(--text-xs)", color: "var(--agent)", fontWeight: 700 }}>
                                Tool: {message.toolName}
                            </span>
                            <span style={{ fontSize: 10, color: "var(--text-muted)", border: "1px solid var(--glass-border)", padding: "1px 6px" }}>
                                {toolStatusLabel}
                            </span>
                        </div>

                        {message.toolArguments && (
                            <pre style={{
                                margin: 0,
                                padding: "8px",
                                background: "rgba(255,255,255,0.04)",
                                border: "1px solid rgba(255,255,255,0.08)",
                                fontSize: 11,
                                lineHeight: 1.4,
                                whiteSpace: "pre-wrap",
                                wordBreak: "break-word",
                                fontFamily: "var(--font-mono)",
                            }}>
                                {(() => {
                                    try {
                                        return JSON.stringify(JSON.parse(message.toolArguments), null, 2);
                                    } catch {
                                        return message.toolArguments;
                                    }
                                })()}
                            </pre>
                        )}

                        {message.content && (
                            <div style={{
                                padding: "8px",
                                background: "rgba(2, 10, 18, 0.55)",
                                border: "1px solid rgba(120, 168, 209, 0.22)",
                                fontSize: 12,
                                lineHeight: 1.45,
                                whiteSpace: "pre-wrap",
                                wordBreak: "break-word",
                            }}>
                                {message.content}
                            </div>
                        )}
                    </div>
                ) : (
                    <MarkdownContent content={displayContent} />
                )}

                {message.isStreaming && (
                    <span style={{ opacity: 0.5, marginLeft: 4 }}>▌</span>
                )}

                {message.model && !isUser && (!isAssistant || hovered) && (
                    <div
                        style={{
                            fontSize: "var(--text-xs)",
                            color: "var(--text-muted)",
                            marginTop: "var(--space-1)",
                        }}
                    >
                        {message.provider}/{message.model}
                    </div>
                )}

                {isAssistant && !message.isStreaming && (message.totalTokens > 0 || message.cost !== undefined || message.tps !== undefined) && (
                    <div
                        style={{
                            fontSize: 11,
                            color: "var(--text-muted)",
                            marginTop: 4,
                            display: "flex",
                            flexWrap: "wrap",
                            gap: 10,
                            opacity: hovered ? 1 : 0,
                            maxHeight: hovered ? 40 : 0,
                            overflow: "hidden",
                            transform: hovered ? "translateY(0)" : "translateY(-4px)",
                            transition: "opacity 180ms ease, max-height 240ms ease, transform 180ms ease",
                            pointerEvents: hovered ? "auto" : "none",
                        }}
                    >
                        <span>∑ {message.totalTokens.toLocaleString()} (⇅ {message.inputTokens.toLocaleString()} / {message.outputTokens.toLocaleString()})</span>
                        {message.reasoningTokens !== undefined && <span>🧠 {message.reasoningTokens}</span>}
                        {message.audioTokens !== undefined && message.audioTokens > 0 && <span>🎵 {message.audioTokens}</span>}
                        {message.videoTokens !== undefined && message.videoTokens > 0 && <span>🎥 {message.videoTokens}</span>}
                        {message.cost !== undefined && <span>${message.cost.toFixed(6)}</span>}
                        {message.tps !== undefined && Number.isFinite(message.tps) && <span>↯ {message.tps.toFixed(1)} tok/s</span>}
                    </div>
                )}

                {hovered && !message.isStreaming && (
                    <div style={{
                        position: "absolute",
                        top: -28,
                        right: isUser ? 0 : undefined,
                        left: isUser ? undefined : 0,
                        display: "flex",
                        gap: 2,
                        background: "var(--bg-secondary)",
                        border: "1px solid var(--glass-border)",
                        borderRadius: "var(--radius-sm)",
                        padding: 2,
                        boxShadow: "var(--shadow-md)",
                    }}>
                        <ActionBtn label={copied ? "Copied!" : "Copy"} onClick={handleCopy} />
                        {isUser && onRerun && <ActionBtn label="Rerun" onClick={onRerun} />}
                        {isAssistant && onRegenerate && <ActionBtn label="Regen" onClick={onRegenerate} />}
                    </div>
                )}
            </div>
        </div>
    );
}

function ActionBtn({ label, onClick }: { label: string; onClick: () => void }) {
    return (
        <button
            onClick={(e) => { e.stopPropagation(); onClick(); }}
            style={{
                background: "transparent",
                border: "none",
                color: "var(--text-muted)",
                cursor: "pointer",
                fontSize: 10,
                fontWeight: 600,
                padding: "3px 6px",
                borderRadius: "var(--radius-sm)",
                transition: "color var(--transition-fast)",
                whiteSpace: "nowrap",
            }}
            onMouseEnter={(e) => { e.currentTarget.style.color = "var(--text-primary)"; e.currentTarget.style.background = "rgba(255,255,255,0.06)"; }}
            onMouseLeave={(e) => { e.currentTarget.style.color = "var(--text-muted)"; e.currentTarget.style.background = "transparent"; }}
        >
            {label}
        </button>
    );
}