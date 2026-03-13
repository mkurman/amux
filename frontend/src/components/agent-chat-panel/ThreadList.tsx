import { useEffect, useMemo, useState } from "react";
import type { AgentThread } from "../../lib/agentStore";
import { DEFAULT_PAGE_SIZE, iconButtonStyle, inputStyle, PageSizeSelect, PaginationControls } from "./shared";

export function ThreadList({
    threads,
    searchQuery,
    onSearch,
    onSelect,
    onDelete,
}: {
    threads: AgentThread[];
    searchQuery: string;
    onSearch: (q: string) => void;
    onSelect: (t: AgentThread) => void;
    onDelete: (id: string) => void;
}) {
    const [pageSize, setPageSize] = useState(DEFAULT_PAGE_SIZE);
    const [page, setPage] = useState(1);
    const [dateFilter, setDateFilter] = useState("");

    const filteredThreads = useMemo(() => {
        return threads.filter((thread) => {
            if (!dateFilter) return true;
            return new Date(thread.updatedAt).toISOString().slice(0, 10) === dateFilter;
        });
    }, [dateFilter, threads]);

    useEffect(() => {
        setPage(1);
    }, [threads, searchQuery, dateFilter, pageSize]);

    const visibleThreads = useMemo(() => {
        const start = (page - 1) * pageSize;
        return filteredThreads.slice(start, start + pageSize);
    }, [filteredThreads, page, pageSize]);

    return (
        <div style={{ height: "100%", overflow: "auto", padding: "var(--space-3)" }}>
            <div style={{ marginBottom: "var(--space-3)", display: "flex", gap: "var(--space-3)", flexWrap: "wrap", alignItems: "center" }}>
                <input
                    type="text"
                    value={searchQuery}
                    onChange={(e) => onSearch(e.target.value)}
                    placeholder="Search threads..."
                    style={{ ...inputStyle, minWidth: 220 }}
                />
                <input
                    type="date"
                    value={dateFilter}
                    onChange={(event) => setDateFilter(event.target.value)}
                    style={{ ...inputStyle, flex: "0 0 auto", minWidth: 170 }}
                />
                <PageSizeSelect value={pageSize} onChange={setPageSize} />
            </div>

            {filteredThreads.length === 0 && (
                <div className="amux-empty-state">
                    <div className="amux-empty-state__icon">💬</div>
                    <div className="amux-empty-state__title">No conversations yet</div>
                    <div className="amux-empty-state__description">Create a new thread to start collaborating with the agent</div>
                </div>
            )}

            <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
                {visibleThreads.map((t) => (
                    <div
                        key={t.id}
                        onClick={() => onSelect(t)}
                        style={{
                            padding: "var(--space-3)",
                            borderRadius: "var(--radius-lg)",
                            border: "1px solid var(--glass-border)",
                            background: "var(--bg-secondary)",
                            cursor: "pointer",
                            transition: "all var(--transition-fast)",
                        }}
                        onMouseEnter={(e) => {
                            e.currentTarget.style.borderColor = "var(--border-strong)";
                            e.currentTarget.style.background = "var(--bg-tertiary)";
                        }}
                        onMouseLeave={(e) => {
                            e.currentTarget.style.borderColor = "var(--glass-border)";
                            e.currentTarget.style.background = "var(--bg-secondary)";
                        }}
                    >
                        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
                            <div style={{ flex: 1, minWidth: 0 }}>
                                <div
                                    style={{
                                        fontSize: "var(--text-sm)",
                                        fontWeight: 600,
                                        overflow: "hidden",
                                        textOverflow: "ellipsis",
                                        whiteSpace: "nowrap",
                                    }}
                                >
                                    {t.title}
                                </div>

                                {t.lastMessagePreview && (
                                    <div
                                        style={{
                                            fontSize: "var(--text-xs)",
                                            color: "var(--text-muted)",
                                            overflow: "hidden",
                                            textOverflow: "ellipsis",
                                            whiteSpace: "nowrap",
                                            marginTop: "var(--space-1)",
                                        }}
                                    >
                                        {t.lastMessagePreview}
                                    </div>
                                )}

                                <div
                                    style={{
                                        fontSize: "var(--text-xs)",
                                        color: "var(--text-muted)",
                                        marginTop: "var(--space-2)",
                                    }}
                                >
                                    {t.messageCount} msgs · {new Date(t.updatedAt).toLocaleDateString()}
                                </div>
                            </div>

                            <button
                                onClick={(e) => {
                                    e.stopPropagation();
                                    onDelete(t.id);
                                }}
                                style={{
                                    ...iconButtonStyle,
                                    color: "var(--danger)",
                                    flexShrink: 0,
                                }}
                                title="Delete thread"
                            >
                                ✕
                            </button>
                        </div>
                    </div>
                ))}
            </div>

            <div style={{ marginTop: "var(--space-3)" }}>
                <PaginationControls
                    page={page}
                    pageSize={pageSize}
                    totalItems={filteredThreads.length}
                    onPageChange={setPage}
                />
            </div>
        </div>
    );
}