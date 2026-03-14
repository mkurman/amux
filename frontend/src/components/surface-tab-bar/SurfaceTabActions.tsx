import { ActionButton } from "./SurfaceTabButton";

export function SurfaceTabActions({
    layoutMode,
    splitActive,
    duplicateSplit,
    applyPresetLayout,
    equalizeLayout,
    toggleZoom,
    toggleWebBrowser,
}: {
    layoutMode: "bsp" | "canvas";
    splitActive: (direction: "horizontal" | "vertical") => void;
    duplicateSplit: (direction: "horizontal" | "vertical") => void;
    applyPresetLayout: (preset: "2-columns" | "grid-2x2" | "main-stack") => void;
    equalizeLayout: () => void;
    toggleZoom: () => void;
    toggleWebBrowser: () => void;
}) {
    if (layoutMode === "canvas") {
        return (
            <div style={{ display: "flex", alignItems: "center", gap: "var(--space-1)" }}>
                <ActionButton title="Toggle browser" onClick={toggleWebBrowser}>WEB</ActionButton>
            </div>
        );
    }

    return (
        <div style={{ display: "flex", alignItems: "center", gap: "var(--space-1)" }}>
            <ActionButton title="Split right" onClick={() => splitActive("horizontal")}>⇄</ActionButton>
            <ActionButton title="Split down" onClick={() => splitActive("vertical")}>⇅</ActionButton>
            <ActionButton title="Duplicate pane to the right" onClick={() => duplicateSplit("horizontal")}>D&gt;</ActionButton>
            <ActionButton title="Duplicate pane downward" onClick={() => duplicateSplit("vertical")}>Dv</ActionButton>
            <ActionButton title="2-column layout" onClick={() => applyPresetLayout("2-columns")}>2C</ActionButton>
            <ActionButton title="Grid layout" onClick={() => applyPresetLayout("grid-2x2")}>▦</ActionButton>
            <ActionButton title="Main + stack layout" onClick={() => applyPresetLayout("main-stack")}>◫</ActionButton>
            <ActionButton title="Equalize ratios" onClick={equalizeLayout}>═</ActionButton>
            <ActionButton title="Toggle zoom" onClick={toggleZoom}>⛶</ActionButton>
            <ActionButton title="Toggle browser" onClick={toggleWebBrowser}>WEB</ActionButton>
        </div>
    );
}
