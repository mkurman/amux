export function SidebarResizeHandle() {
    return (
        <div
            data-sidebar-resize-handle="true"
            style={{
                position: "absolute",
                top: 0,
                right: 0,
                width: 8,
                height: "100%",
                cursor: "col-resize",
                zIndex: 20,
                background: "linear-gradient(90deg, transparent, rgba(148, 163, 184, 0.35), transparent)",
            }}
        />
    );
}
