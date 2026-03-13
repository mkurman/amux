import { useEffect, useMemo, useState } from "react";
import { ReasoningStream } from "../ReasoningStream";
import { useAgentMissionStore } from "../../lib/agentMissionStore";
import { DEFAULT_PAGE_SIZE, EmptyPanel, MetricRibbon, PageSizeSelect, PaginationControls, SectionTitle, inputStyle } from "./shared";

export function TraceView({
    operationalEvents,
    cognitiveEvents,
    pendingApprovals,
}: {
    operationalEvents: ReturnType<typeof useAgentMissionStore.getState>["operationalEvents"];
    cognitiveEvents: ReturnType<typeof useAgentMissionStore.getState>["cognitiveEvents"];
    pendingApprovals: ReturnType<typeof useAgentMissionStore.getState>["approvals"];
}) {
    const [searchQuery, setSearchQuery] = useState("");
    const [dateFilter, setDateFilter] = useState("");
    const [pageSize, setPageSize] = useState(DEFAULT_PAGE_SIZE);
    const [reasoningPage, setReasoningPage] = useState(1);
    const [timelinePage, setTimelinePage] = useState(1);

    const matchesDate = (timestamp: number) => {
        if (!dateFilter) return true;
        return new Date(timestamp).toISOString().slice(0, 10) === dateFilter;
    };

    const normalizedQuery = searchQuery.trim().toLowerCase();

    const filteredCognitiveEvents = useMemo(() => {
        return cognitiveEvents.filter((event) => {
            if (!matchesDate(event.timestamp)) return false;
            if (!normalizedQuery) return true;

            return [event.source, event.content]
                .join(" ")
                .toLowerCase()
                .includes(normalizedQuery);
        });
    }, [cognitiveEvents, dateFilter, normalizedQuery]);

    const filteredOperationalEvents = useMemo(() => {
        return operationalEvents.filter((event) => {
            if (!matchesDate(event.timestamp)) return false;
            if (!normalizedQuery) return true;

            return [event.kind, event.command ?? "", event.message ?? "", event.blastRadius ?? ""]
                .join(" ")
                .toLowerCase()
                .includes(normalizedQuery);
        });
    }, [operationalEvents, dateFilter, normalizedQuery]);

    useEffect(() => {
        setReasoningPage(1);
        setTimelinePage(1);
    }, [searchQuery, dateFilter, pageSize]);

    const visibleCognitiveEvents = useMemo(() => {
        const start = (reasoningPage - 1) * pageSize;
        return filteredCognitiveEvents.slice(start, start + pageSize);
    }, [filteredCognitiveEvents, pageSize, reasoningPage]);

    const visibleOperationalEvents = useMemo(() => {
        const start = (timelinePage - 1) * pageSize;
        return filteredOperationalEvents.slice(start, start + pageSize);
    }, [filteredOperationalEvents, pageSize, timelinePage]);

    return (
        <div style={{ padding: "var(--space-4)", height: "100%", overflow: "auto" }}>
            <MetricRibbon
                items={[
                    { label: "Ops", value: String(operationalEvents.length) },
                    { label: "Reasoning", value: String(cognitiveEvents.length) },
                    { label: "Pending", value: String(pendingApprovals.length) },
                ]}
            />

            <div style={{ display: "flex", gap: "var(--space-3)", flexWrap: "wrap", alignItems: "center", marginBottom: "var(--space-4)" }}>
                <input
                    type="text"
                    value={searchQuery}
                    onChange={(event) => setSearchQuery(event.target.value)}
                    placeholder="Search trace and timeline..."
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

            <SectionTitle title="Reasoning Trace" subtitle="Parsed cognitive events" />
            <ReasoningStream events={visibleCognitiveEvents} />
            <div style={{ marginTop: "var(--space-3)" }}>
                <PaginationControls
                    page={reasoningPage}
                    pageSize={pageSize}
                    totalItems={filteredCognitiveEvents.length}
                    onPageChange={setReasoningPage}
                />
            </div>

            <SectionTitle title="Operational Timeline" subtitle="Execution events" />

            {visibleOperationalEvents.length === 0 ? (
                <EmptyPanel message="No operational events captured yet." />
            ) : (
                <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
                    {visibleOperationalEvents.map((event) => (
                        <TimelineRow key={event.id} event={event} />
                    ))}
                </div>
            )}

            <div style={{ marginTop: "var(--space-3)" }}>
                <PaginationControls
                    page={timelinePage}
                    pageSize={pageSize}
                    totalItems={filteredOperationalEvents.length}
                    onPageChange={setTimelinePage}
                />
            </div>
        </div>
    );
}

function TimelineRow({ event }: { event: ReturnType<typeof useAgentMissionStore.getState>["operationalEvents"][number] }) {
    return (
        <div
            style={{
                display: "flex",
                gap: "var(--space-3)",
                padding: "var(--space-3)",
                borderRadius: "var(--radius-lg)",
                border: "1px solid var(--glass-border)",
                background: "var(--bg-secondary)",
                alignItems: "flex-start",
            }}
        >
            <div style={{ minWidth: 72, fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>
                {new Date(event.timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
            </div>

            <div style={{ flex: 1 }}>
                <div style={{ fontSize: "var(--text-sm)", fontWeight: 600 }}>{event.kind}</div>

                {event.command && (
                    <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginTop: "var(--space-1)" }}>
                        {event.command}
                    </div>
                )}
            </div>

            {event.exitCode !== null && (
                <div
                    style={{
                        fontSize: "var(--text-xs)",
                        color: event.exitCode === 0 ? "var(--success)" : "var(--danger)",
                    }}
                >
                    {event.exitCode === 0 ? "✓" : `✗ ${event.exitCode}`}
                </div>
            )}
        </div>
    );
}