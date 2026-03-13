import type React from "react";

export const DEFAULT_PAGE_SIZE = 5;
export const PAGE_SIZE_OPTIONS = [5, 10, 20, 50];

export function MetricRibbon({ items }: { items: Array<{ label: string; value: string; accent?: string }> }) {
    return (
        <div
            style={{
                display: "grid",
                gridTemplateColumns: `repeat(${Math.min(2, items.length)}, minmax(0, 1fr))`,
                gap: "var(--space-3)",
                marginBottom: "var(--space-4)",
            }}
        >
            {items.map((item) => (
                <div
                    key={item.label}
                    style={{
                        padding: "var(--space-3)",
                        borderRadius: "var(--radius-lg)",
                        border: "1px solid var(--glass-border)",
                        background: "var(--bg-secondary)",
                    }}
                >
                    <div className="amux-panel-title">{item.label}</div>
                    <div
                        style={{
                            fontSize: "var(--text-md)",
                            fontWeight: 700,
                            marginTop: "var(--space-1)",
                            color: item.accent || "var(--text-primary)",
                        }}
                    >
                        {item.value}
                    </div>
                </div>
            ))}
        </div>
    );
}

export function SectionTitle({ title, subtitle }: { title: string; subtitle: string }) {
    return (
        <div style={{ marginBottom: "var(--space-3)", marginTop: "var(--space-4)" }}>
            <div style={{ fontSize: "var(--text-sm)", fontWeight: 600 }}>{title}</div>
            <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", marginTop: 2 }}>{subtitle}</div>
        </div>
    );
}

export function EmptyPanel({ message }: { message: string }) {
    return (
        <div
            style={{
                padding: "var(--space-6)",
                borderRadius: "var(--radius-lg)",
                border: "1px dashed var(--border)",
                color: "var(--text-muted)",
                fontSize: "var(--text-sm)",
                textAlign: "center",
                background: "var(--bg-secondary)",
            }}
        >
            {message}
        </div>
    );
}

export function ContextCard({ label, value, href }: { label: string; value: string; href?: string }) {
    return (
        <div
            style={{
                padding: "var(--space-3)",
                borderRadius: "var(--radius-lg)",
                border: "1px solid var(--glass-border)",
                background: "var(--bg-secondary)",
            }}
        >
            <div className="amux-panel-title">{label}</div>
            {href ? (
                <a
                    href={href}
                    target="_blank"
                    rel="noreferrer"
                    style={{
                        display: "block",
                        fontSize: "var(--text-sm)",
                        marginTop: "var(--space-1)",
                        wordBreak: "break-word",
                        color: "var(--accent)",
                        textDecoration: "underline",
                        textUnderlineOffset: 2,
                    }}
                >
                    {value}
                </a>
            ) : (
                <div style={{ fontSize: "var(--text-sm)", marginTop: "var(--space-1)", wordBreak: "break-word" }}>{value}</div>
            )}
        </div>
    );
}

export function ActionButton({ children, onClick }: { children: React.ReactNode; onClick?: () => void }) {
    return (
        <button
            type="button"
            onClick={onClick}
            style={{
                padding: "var(--space-2) var(--space-3)",
                borderRadius: "var(--radius-md)",
                border: "1px solid var(--border)",
                background: "var(--bg-tertiary)",
                color: "var(--text-secondary)",
                fontSize: "var(--text-xs)",
                cursor: "pointer",
                transition: "all var(--transition-fast)",
            }}
            onMouseEnter={(e) => {
                e.currentTarget.style.borderColor = "var(--border-strong)";
                e.currentTarget.style.color = "var(--text-primary)";
            }}
            onMouseLeave={(e) => {
                e.currentTarget.style.borderColor = "var(--border)";
                e.currentTarget.style.color = "var(--text-secondary)";
            }}
        >
            {children}
        </button>
    );
}

export const iconButtonStyle: React.CSSProperties = {
    background: "var(--bg-secondary)",
    border: "1px solid var(--glass-border)",
    color: "var(--text-muted)",
    cursor: "pointer",
    fontSize: "var(--text-sm)",
    padding: "var(--space-1) var(--space-2)",
    borderRadius: "var(--radius-md)",
    transition: "all var(--transition-fast)",
};

export const memoryAreaStyle: React.CSSProperties = {
    width: "100%",
    minHeight: 160,
    borderRadius: "var(--radius-lg)",
    border: "1px solid var(--glass-border)",
    background: "var(--bg-secondary)",
    color: "var(--text-primary)",
    padding: "var(--space-3)",
    resize: "vertical",
    fontSize: "var(--text-sm)",
    lineHeight: 1.6,
    fontFamily: "var(--font-mono)",
};

export const inputStyle: React.CSSProperties = {
    flex: 1,
    minWidth: 0,
    borderRadius: "var(--radius-md)",
    border: "1px solid var(--glass-border)",
    background: "var(--bg-secondary)",
    color: "var(--text-primary)",
    padding: "var(--space-2) var(--space-3)",
    fontSize: "var(--text-sm)",
};

export function PageSizeSelect({
    value,
    onChange,
    label = "Per page",
}: {
    value: number;
    onChange: (value: number) => void;
    label?: string;
}) {
    return (
        <label style={{ display: "flex", alignItems: "center", gap: 8, fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>
            <span>{label}</span>
            <select
                value={value}
                onChange={(event) => onChange(Number(event.target.value))}
                style={{
                    ...inputStyle,
                    flex: "0 0 auto",
                    width: 88,
                    padding: "6px 8px",
                    fontSize: "var(--text-xs)",
                }}
            >
                {PAGE_SIZE_OPTIONS.map((option) => (
                    <option key={option} value={option}>{option}</option>
                ))}
            </select>
        </label>
    );
}

export function PaginationControls({
    page,
    pageSize,
    totalItems,
    onPageChange,
}: {
    page: number;
    pageSize: number;
    totalItems: number;
    onPageChange: (page: number) => void;
}) {
    const totalPages = Math.max(1, Math.ceil(totalItems / Math.max(1, pageSize)));

    if (totalItems <= pageSize) {
        return (
            <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>
                {totalItems} item{totalItems === 1 ? "" : "s"}
            </div>
        );
    }

    return (
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "var(--space-3)", flexWrap: "wrap" }}>
            <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)" }}>
                Page {page} of {totalPages} · {totalItems} items
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
                <ActionButton onClick={page > 1 ? () => onPageChange(page - 1) : undefined}>
                    Prev
                </ActionButton>
                <ActionButton onClick={page < totalPages ? () => onPageChange(page + 1) : undefined}>
                    Next
                </ActionButton>
            </div>
        </div>
    );
}