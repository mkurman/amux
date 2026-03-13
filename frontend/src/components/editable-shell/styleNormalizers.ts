import type React from "react";

export interface NormalizeWrapperStyleInput {
    style?: React.CSSProperties;
    resizable?: boolean;
    resizeAxis?: "both" | "horizontal" | "vertical";
    minWidth?: number | string;
    minHeight?: number | string;
    maxWidth?: number | string;
    maxHeight?: number | string;
}

export function shouldPreserveFillLayout(style: React.CSSProperties): boolean {
    return style.display === undefined && (
        style.flex !== undefined
        || style.height === "100%"
        || style.minHeight !== undefined
        || style.maxHeight !== undefined
    );
}

export function normalizeWrapperStyle({
    style,
    resizable,
    minWidth,
    minHeight,
    maxWidth,
    maxHeight,
}: NormalizeWrapperStyleInput): React.CSSProperties {
    const wrapperStyle: React.CSSProperties = {
        ...(style ?? {}),
    };

    if (resizable && wrapperStyle.overflow === undefined) {
        wrapperStyle.overflow = "auto";
    }

    if (minWidth !== undefined) wrapperStyle.minWidth = minWidth;
    if (minHeight !== undefined) wrapperStyle.minHeight = minHeight;
    if (maxWidth !== undefined) wrapperStyle.maxWidth = maxWidth;
    if (maxHeight !== undefined) wrapperStyle.maxHeight = maxHeight;

    if (shouldPreserveFillLayout(wrapperStyle)) {
        wrapperStyle.display = "flex";
        if (wrapperStyle.flexDirection === undefined) {
            wrapperStyle.flexDirection = "column";
        }
    }

    return wrapperStyle;
}