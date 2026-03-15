import { useCallback, useEffect, useState } from "react";
import { SectionTitle, ActionButton, iconButtonStyle } from "./shared";

interface AgentTask {
    id: string;
    title: string;
    description: string;
    status: "queued" | "running" | "completed" | "failed" | "cancelled";
    priority: string;
    progress: number;
    created_at: number;
    started_at: number | null;
    completed_at: number | null;
    error: string | null;
    result: string | null;
    thread_id: string | null;
    source: string;
}

interface HeartbeatItem {
    id: string;
    label: string;
    prompt: string;
    interval_minutes: number;
    enabled: boolean;
    last_run_at: number | null;
    last_result: "ok" | "alert" | "error" | null;
    last_message: string | null;
}

const statusColors: Record<string, string> = {
    queued: "var(--text-muted)",
    running: "var(--accent)",
    completed: "var(--success)",
    failed: "var(--danger)",
    cancelled: "var(--text-muted)",
};

const heartbeatColors: Record<string, string> = {
    ok: "var(--success)",
    alert: "var(--warning)",
    error: "var(--danger)",
};

export function TasksView() {
    const [tasks, setTasks] = useState<AgentTask[]>([]);
    const [heartbeatItems, setHeartbeatItems] = useState<HeartbeatItem[]>([]);
    const [newTaskTitle, setNewTaskTitle] = useState("");
    const [newTaskDescription, setNewTaskDescription] = useState("");

    const amux = (window as any).tamux ?? (window as any).amux;

    const refreshTasks = useCallback(async () => {
        if (!amux?.agentListTasks) return;
        try {
            const result = await amux.agentListTasks();
            setTasks(Array.isArray(result) ? result : []);
        } catch { /* silent */ }
    }, [amux]);

    const refreshHeartbeat = useCallback(async () => {
        if (!amux?.agentHeartbeatGetItems) return;
        try {
            const result = await amux.agentHeartbeatGetItems();
            setHeartbeatItems(Array.isArray(result) ? result : []);
        } catch { /* silent */ }
    }, [amux]);

    useEffect(() => {
        refreshTasks();
        refreshHeartbeat();
        const interval = setInterval(() => {
            refreshTasks();
            refreshHeartbeat();
        }, 5000);
        return () => clearInterval(interval);
    }, [refreshTasks, refreshHeartbeat]);

    const addTask = async () => {
        if (!newTaskTitle.trim() || !amux?.agentAddTask) return;
        await amux.agentAddTask(newTaskTitle, newTaskDescription || newTaskTitle, "normal");
        setNewTaskTitle("");
        setNewTaskDescription("");
        refreshTasks();
    };

    const cancelTask = async (taskId: string) => {
        if (!amux?.agentCancelTask) return;
        await amux.agentCancelTask(taskId);
        refreshTasks();
    };

    const activeTasks = tasks.filter((t) => t.status === "queued" || t.status === "running");
    const completedTasks = tasks.filter((t) => t.status === "completed" || t.status === "failed" || t.status === "cancelled");

    return (
        <div style={{ padding: "var(--space-4)", overflow: "auto", height: "100%" }}>
            <SectionTitle title="Task Queue" subtitle="Autonomous task execution by daemon agent" />

            {/* Add task form */}
            <div style={{ marginBottom: "var(--space-4)", display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
                <input
                    type="text"
                    placeholder="Task title..."
                    value={newTaskTitle}
                    onChange={(e) => setNewTaskTitle(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && addTask()}
                    style={{
                        padding: "var(--space-2) var(--space-3)",
                        borderRadius: "var(--radius-md)",
                        border: "1px solid var(--border)",
                        background: "var(--bg-tertiary)",
                        color: "var(--text-primary)",
                        fontSize: "var(--text-sm)",
                        outline: "none",
                    }}
                />
                {newTaskTitle && (
                    <textarea
                        placeholder="Description (optional)..."
                        value={newTaskDescription}
                        onChange={(e) => setNewTaskDescription(e.target.value)}
                        rows={2}
                        style={{
                            padding: "var(--space-2) var(--space-3)",
                            borderRadius: "var(--radius-md)",
                            border: "1px solid var(--border)",
                            background: "var(--bg-tertiary)",
                            color: "var(--text-primary)",
                            fontSize: "var(--text-xs)",
                            outline: "none",
                            resize: "vertical",
                            fontFamily: "inherit",
                        }}
                    />
                )}
                {newTaskTitle && (
                    <ActionButton onClick={addTask}>Add Task</ActionButton>
                )}
            </div>

            {/* Active tasks */}
            {activeTasks.length > 0 && (
                <div style={{ marginBottom: "var(--space-4)" }}>
                    <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginBottom: "var(--space-2)", fontWeight: 600 }}>
                        Active ({activeTasks.length})
                    </div>
                    {activeTasks.map((task) => (
                        <TaskCard key={task.id} task={task} onCancel={() => cancelTask(task.id)} />
                    ))}
                </div>
            )}

            {/* Completed tasks */}
            {completedTasks.length > 0 && (
                <div style={{ marginBottom: "var(--space-4)" }}>
                    <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginBottom: "var(--space-2)", fontWeight: 600 }}>
                        History ({completedTasks.length})
                    </div>
                    {completedTasks.slice(0, 20).map((task) => (
                        <TaskCard key={task.id} task={task} />
                    ))}
                </div>
            )}

            {tasks.length === 0 && (
                <div style={{ textAlign: "center", padding: "var(--space-6)", color: "var(--text-muted)", fontSize: "var(--text-sm)" }}>
                    No tasks yet. Add a task above or tell the agent what to do.
                </div>
            )}

            {/* Heartbeat section */}
            <SectionTitle title="Heartbeat" subtitle="Periodic health checks run by the daemon" />

            {heartbeatItems.length > 0 ? (
                heartbeatItems.map((item) => (
                    <HeartbeatCard key={item.id} item={item} />
                ))
            ) : (
                <div style={{ textAlign: "center", padding: "var(--space-4)", color: "var(--text-muted)", fontSize: "var(--text-xs)" }}>
                    No heartbeat checks configured. Edit ~/.tamux/agent/heartbeat.json to add checks.
                </div>
            )}
        </div>
    );
}

function TaskCard({ task, onCancel }: { task: AgentTask; onCancel?: () => void }) {
    const statusColor = statusColors[task.status] || "var(--text-muted)";
    const isActive = task.status === "queued" || task.status === "running";

    return (
        <div
            style={{
                padding: "var(--space-3)",
                borderRadius: "var(--radius-md)",
                border: "1px solid var(--border)",
                background: "var(--bg-secondary)",
                marginBottom: "var(--space-2)",
            }}
        >
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "var(--space-2)" }}>
                <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontSize: "var(--text-sm)", fontWeight: 500, color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {task.title}
                    </div>
                    <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginTop: 2 }}>
                        <span style={{ color: statusColor, fontWeight: 600 }}>{task.status}</span>
                        {task.status === "running" && task.progress > 0 && (
                            <span> {task.progress}%</span>
                        )}
                        <span style={{ marginLeft: "var(--space-2)" }}>
                            {new Date(task.created_at).toLocaleTimeString()}
                        </span>
                    </div>
                </div>
                {isActive && onCancel && (
                    <button type="button" onClick={onCancel} style={{ ...iconButtonStyle, fontSize: 11 }} title="Cancel task">
                        Cancel
                    </button>
                )}
            </div>
            {task.error && (
                <div style={{ fontSize: "var(--text-xs)", color: "var(--danger)", marginTop: "var(--space-2)" }}>
                    {task.error}
                </div>
            )}
        </div>
    );
}

function HeartbeatCard({ item }: { item: HeartbeatItem }) {
    const resultColor = item.last_result ? (heartbeatColors[item.last_result] || "var(--text-muted)") : "var(--text-muted)";

    return (
        <div
            style={{
                padding: "var(--space-3)",
                borderRadius: "var(--radius-md)",
                border: "1px solid var(--border)",
                background: "var(--bg-secondary)",
                marginBottom: "var(--space-2)",
                opacity: item.enabled ? 1 : 0.5,
            }}
        >
            <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
                <div
                    style={{
                        width: 8,
                        height: 8,
                        borderRadius: "50%",
                        background: resultColor,
                        flexShrink: 0,
                    }}
                />
                <div style={{ flex: 1 }}>
                    <div style={{ fontSize: "var(--text-sm)", fontWeight: 500, color: "var(--text-primary)" }}>
                        {item.label}
                    </div>
                    <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginTop: 1 }}>
                        {item.last_run_at
                            ? `Last: ${new Date(item.last_run_at).toLocaleTimeString()}`
                            : "Never run"}
                        {item.interval_minutes > 0 && ` (every ${item.interval_minutes}m)`}
                    </div>
                </div>
            </div>
            {item.last_message && item.last_result !== "ok" && (
                <div style={{ fontSize: "var(--text-xs)", color: resultColor, marginTop: "var(--space-2)", paddingLeft: 16 }}>
                    {item.last_message.slice(0, 200)}
                </div>
            )}
        </div>
    );
}
