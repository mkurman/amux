import { useCallback } from "react";
import type { MutableRefObject } from "react";
import type { Terminal } from "@xterm/xterm";

export function useTerminalClipboard({
    termRef,
    sendTextInput,
}: {
    termRef: MutableRefObject<Terminal | null>;
    sendTextInput: (text: string, options?: { bracketed?: boolean; trackHistory?: boolean }) => Promise<boolean>;
}) {
    const writeClipboardText = useCallback(async (text: string) => {
        const amux = (window as any).tamux ?? (window as any).amux;
        if (amux?.writeClipboardText) {
            await amux.writeClipboardText(text);
            return;
        }

        if (navigator.clipboard?.writeText) {
            await navigator.clipboard.writeText(text);
        }
    }, []);

    const readClipboardText = useCallback(async (): Promise<string> => {
        const amux = (window as any).tamux ?? (window as any).amux;
        if (amux?.readClipboardText) {
            return (await amux.readClipboardText()) ?? "";
        }

        if (navigator.clipboard?.readText) {
            return navigator.clipboard.readText();
        }

        return "";
    }, []);

    const copySelection = useCallback(async () => {
        const term = termRef.current;
        if (!term || !term.hasSelection()) return;

        const selection = term.getSelection();
        if (!selection) return;

        await writeClipboardText(selection);
        term.clearSelection();
    }, [termRef, writeClipboardText]);

    const pasteClipboard = useCallback(async () => {
        const text = await readClipboardText();
        if (!text) return;

        await sendTextInput(text, { bracketed: true, trackHistory: true });
    }, [readClipboardText, sendTextInput]);

    return {
        writeClipboardText,
        readClipboardText,
        copySelection,
        pasteClipboard,
    };
}