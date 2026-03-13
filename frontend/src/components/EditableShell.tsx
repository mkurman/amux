import { useMemo, useRef, useState } from "react";
import type React from "react";
import type { UINodeBuilderMeta } from "../schemas/uiSchema";
import { EditableShellChrome } from "./editable-shell/EditableShellChrome";
import { normalizeWrapperStyle } from "./editable-shell/styleNormalizers";
import { useEditableShellDragDrop } from "./editable-shell/useEditableShellDragDrop";
import { useEditableShellState } from "./editable-shell/useEditableShellState";

interface EditableShellProps {
    style?: React.CSSProperties;
    className?: string;
    children?: React.ReactNode;
    content: React.ReactNode;
    visible?: boolean;
    hidden?: boolean;
    resizable?: boolean;
    resizeAxis?: "both" | "horizontal" | "vertical";
    minWidth?: number | string;
    minHeight?: number | string;
    maxWidth?: number | string;
    maxHeight?: number | string;
    builderNodeId?: string;
    builderViewId?: string;
    builderComponentType?: string;
    builderMeta?: UINodeBuilderMeta;
}

export function EditableShell({
    style,
    className,
    children,
    content,
    visible,
    hidden,
    resizable,
    resizeAxis,
    minWidth,
    minHeight,
    maxWidth,
    maxHeight,
    builderNodeId,
    builderViewId,
    builderComponentType,
    builderMeta,
}: EditableShellProps) {
    const shellRef = useRef<HTMLDivElement | null>(null);
    const [sizeOverride, setSizeOverride] = useState<{ width?: number; height?: number } | null>(null);
    const resolvedResizeAxis = resizeAxis ?? "both";
    const horizontalHandleThickness = 10;
    const verticalHandleThickness = 10;
    const cornerHandleSize = 14;

    if (hidden || visible === false) {
        return null;
    }

    const wrapperStyle = normalizeWrapperStyle({
        style,
        resizable,
        resizeAxis,
        minWidth,
        minHeight,
        maxWidth,
        maxHeight,
    });

    const parseConstraint = (value?: number | string): number | undefined => {
        if (typeof value === "number" && Number.isFinite(value)) {
            return value;
        }

        if (typeof value === "string") {
            const parsed = Number.parseFloat(value);
            return Number.isFinite(parsed) ? parsed : undefined;
        }

        return undefined;
    };

    const minWidthValue = parseConstraint(minWidth);
    const minHeightValue = parseConstraint(minHeight);
    const maxWidthValue = parseConstraint(maxWidth);
    const maxHeightValue = parseConstraint(maxHeight);

    const clamp = (value: number, min?: number, max?: number): number => {
        const boundedMin = min ?? Number.NEGATIVE_INFINITY;
        const boundedMax = max ?? Number.POSITIVE_INFINITY;
        return Math.min(boundedMax, Math.max(boundedMin, value));
    };

    const handleResizeStart = (
        axis: "horizontal" | "vertical" | "both",
        horizontalEdge: "start" | "end" = "end",
        verticalEdge: "start" | "end" = "end",
    ) => (event: React.PointerEvent<HTMLDivElement>) => {
        if (!resizable || !shellRef.current) {
            return;
        }

        event.preventDefault();
        event.stopPropagation();

        const pointerId = event.pointerId;
        const target = event.currentTarget;
        const rect = shellRef.current.getBoundingClientRect();
        const startWidth = rect.width;
        const startHeight = rect.height;
        const startLeft = rect.left;
        const startRight = rect.right;
        const startTop = rect.top;
        const startBottom = rect.bottom;

        setSizeOverride((current) => ({
            width: axis === "vertical" ? current?.width : startWidth,
            height: axis === "horizontal" ? current?.height : startHeight,
        }));

        target.setPointerCapture(pointerId);
        document.body.style.userSelect = "none";
        document.body.style.cursor = axis === "horizontal"
            ? "col-resize"
            : axis === "vertical"
                ? "row-resize"
                : "nwse-resize";

        const onPointerMove = (moveEvent: PointerEvent) => {
            const nextWidth = horizontalEdge === "start"
                ? clamp(startRight - moveEvent.clientX, minWidthValue, maxWidthValue)
                : clamp(moveEvent.clientX - startLeft, minWidthValue, maxWidthValue);
            const nextHeight = verticalEdge === "start"
                ? clamp(startBottom - moveEvent.clientY, minHeightValue, maxHeightValue)
                : clamp(moveEvent.clientY - startTop, minHeightValue, maxHeightValue);

            setSizeOverride((current) => ({
                width: axis === "vertical"
                    ? current?.width
                    : nextWidth,
                height: axis === "horizontal"
                    ? current?.height
                    : nextHeight,
            }));
        };

        const onPointerEnd = () => {
            if (target.hasPointerCapture(pointerId)) {
                target.releasePointerCapture(pointerId);
            }
            document.body.style.userSelect = "";
            document.body.style.cursor = "";
            window.removeEventListener("pointermove", onPointerMove);
            window.removeEventListener("pointerup", onPointerEnd);
            window.removeEventListener("pointercancel", onPointerEnd);
        };

        window.addEventListener("pointermove", onPointerMove);
        window.addEventListener("pointerup", onPointerEnd);
        window.addEventListener("pointercancel", onPointerEnd);
    };
    const {
        chromeEnabled,
        isSelected,
        hasWrapperStyling,
        selectionStyle,
        menuOpen,
        isEditMode,
        handleSelect,
        handleStartEditing,
    } = useEditableShellState({
        className,
        style,
        resizable,
        minWidth,
        minHeight,
        maxWidth,
        maxHeight,
        builderNodeId,
        builderViewId,
        builderComponentType,
        builderMeta,
    });

    if (!chromeEnabled && !isSelected && !hasWrapperStyling) {
        if (children) {
            return (
                <>
                    {content}
                    {children}
                </>
            );
        }

        return <>{content}</>;
    }

    if (wrapperStyle.position === undefined) {
        wrapperStyle.position = "relative";
    }

    const dropEnabled = Boolean(chromeEnabled && isEditMode && builderMeta?.locked !== true);
    const { dropActive, onDragOver, onDragLeave, onDrop } = useEditableShellDragDrop({
        dropEnabled,
        builderNodeId,
    });

    const resolvedWrapperStyle = useMemo<React.CSSProperties>(() => {
        const resolved: React.CSSProperties = {
            ...wrapperStyle,
            ...(sizeOverride?.width !== undefined ? { width: `${sizeOverride.width}px` } : {}),
            ...(sizeOverride?.height !== undefined ? { height: `${sizeOverride.height}px` } : {}),
        };

        if (sizeOverride?.width !== undefined || sizeOverride?.height !== undefined) {
            resolved.flexGrow = 0;
            resolved.flexShrink = 0;

            if (sizeOverride?.width !== undefined && sizeOverride?.height === undefined) {
                resolved.flexBasis = `${sizeOverride.width}px`;
            } else if (sizeOverride?.height !== undefined && sizeOverride?.width === undefined) {
                resolved.flexBasis = `${sizeOverride.height}px`;
            } else {
                resolved.flexBasis = "auto";
            }
        }

        return resolved;
    }, [sizeOverride, wrapperStyle]);

    return (
        <div
            ref={shellRef}
            style={{
                ...resolvedWrapperStyle,
                ...selectionStyle,
                ...(dropActive ? { boxShadow: "0 0 0 2px rgba(129, 230, 217, 0.85) inset" } : {}),
            }}
            className={className}
            onClickCapture={handleSelect}
            onDragOver={onDragOver}
            onDragLeave={onDragLeave}
            onDrop={onDrop}
        >
            {chromeEnabled ? <EditableShellChrome menuOpen={menuOpen} onEdit={handleStartEditing} /> : null}
            {content}
            {children}
            {resizable ? (
                <>
                    {resolvedResizeAxis !== "vertical" ? (
                        <>
                            <div
                                onPointerDown={handleResizeStart("horizontal", "start")}
                                style={{
                                    position: "absolute",
                                    top: 0,
                                    left: 0,
                                    width: horizontalHandleThickness,
                                    height: resolvedResizeAxis === "horizontal" ? "100%" : `calc(100% - ${cornerHandleSize}px)`,
                                    cursor: "col-resize",
                                    zIndex: 20,
                                    background: "linear-gradient(90deg, rgba(148, 163, 184, 0.2), rgba(148, 163, 184, 0.45), transparent)",
                                }}
                            />
                            <div
                                onPointerDown={handleResizeStart("horizontal", "end")}
                                style={{
                                    position: "absolute",
                                    top: 0,
                                    right: 0,
                                    width: horizontalHandleThickness,
                                    height: resolvedResizeAxis === "horizontal" ? "100%" : `calc(100% - ${cornerHandleSize}px)`,
                                    cursor: "col-resize",
                                    zIndex: 20,
                                    background: "linear-gradient(90deg, transparent, rgba(148, 163, 184, 0.45), rgba(148, 163, 184, 0.2))",
                                }}
                            />
                        </>
                    ) : null}
                    {resolvedResizeAxis !== "horizontal" ? (
                        <>
                            <div
                                onPointerDown={handleResizeStart("vertical", "end", "start")}
                                style={{
                                    position: "absolute",
                                    top: 0,
                                    left: 0,
                                    width: resolvedResizeAxis === "vertical" ? "100%" : `calc(100% - ${cornerHandleSize}px)`,
                                    height: verticalHandleThickness,
                                    cursor: "row-resize",
                                    zIndex: 20,
                                    background: "linear-gradient(180deg, rgba(148, 163, 184, 0.2), rgba(148, 163, 184, 0.45), transparent)",
                                }}
                            />
                            <div
                                onPointerDown={handleResizeStart("vertical", "end", "end")}
                                style={{
                                    position: "absolute",
                                    left: 0,
                                    bottom: 0,
                                    width: resolvedResizeAxis === "vertical" ? "100%" : `calc(100% - ${cornerHandleSize}px)`,
                                    height: verticalHandleThickness,
                                    cursor: "row-resize",
                                    zIndex: 20,
                                    background: "linear-gradient(180deg, transparent, rgba(148, 163, 184, 0.45), rgba(148, 163, 184, 0.2))",
                                }}
                            />
                        </>
                    ) : null}
                    {resolvedResizeAxis === "both" ? (
                        <div
                            onPointerDown={handleResizeStart("both", "end", "end")}
                            style={{
                                position: "absolute",
                                right: 0,
                                bottom: 0,
                                width: cornerHandleSize,
                                height: cornerHandleSize,
                                cursor: "nwse-resize",
                                zIndex: 21,
                                background: "linear-gradient(135deg, transparent 0 35%, rgba(148, 163, 184, 0.65) 35% 45%, transparent 45% 55%, rgba(148, 163, 184, 0.65) 55% 65%, transparent 65% 100%)",
                            }}
                        />
                    ) : null}
                </>
            ) : null}
        </div>
    );
}
