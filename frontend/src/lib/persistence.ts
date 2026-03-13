type PendingWrite = {
    relativePath: string;
    kind: "json" | "text" | "delete";
    value?: unknown;
    timer: number | null;
};

const pendingWrites = new Map<string, PendingWrite>();

function getBridge(): AmuxBridge | undefined {
    if (typeof window === "undefined") return undefined;
    return window.amux;
}

function writeKey(kind: PendingWrite["kind"], relativePath: string): string {
    return `${kind}:${relativePath}`;
}

async function flushWrite(pending: PendingWrite): Promise<void> {
    const bridge = getBridge();
    const key = writeKey(pending.kind, pending.relativePath);
    pendingWrites.delete(key);

    if (!bridge) return;

    if (pending.kind === "json" && bridge.writeJsonFile) {
        await bridge.writeJsonFile(pending.relativePath, pending.value);
        return;
    }

    if (pending.kind === "text" && bridge.writeTextFile) {
        await bridge.writeTextFile(pending.relativePath, typeof pending.value === "string" ? pending.value : "");
        return;
    }

    if (pending.kind === "delete" && bridge.deleteDataPath) {
        await bridge.deleteDataPath(pending.relativePath);
    }
}

function scheduleWrite(kind: PendingWrite["kind"], relativePath: string, value: unknown, delayMs: number) {
    if (typeof window === "undefined") return;

    const key = writeKey(kind, relativePath);
    const existing = pendingWrites.get(key);
    if (existing && existing.timer !== null) {
        window.clearTimeout(existing.timer);
    }

    const pending: PendingWrite = {
        relativePath,
        kind,
        value,
        timer: window.setTimeout(() => {
            void flushWrite(pending);
        }, Math.max(0, delayMs)),
    };

    pendingWrites.set(key, pending);
}

if (typeof window !== "undefined") {
    window.addEventListener("beforeunload", () => {
        void flushPendingWrites();
    });
}

export async function getDataDir(): Promise<string | null> {
    return (await getBridge()?.getDataDir?.()) ?? null;
}

export async function readPersistedJson<T>(relativePath: string): Promise<T | null> {
    const value = await getBridge()?.readJsonFile?.(relativePath);
    return value === undefined ? null : (value as T | null);
}

export async function readPersistedText(relativePath: string): Promise<string | null> {
    return (await getBridge()?.readTextFile?.(relativePath)) ?? null;
}

export function scheduleJsonWrite(relativePath: string, data: unknown, delayMs = 300): void {
    scheduleWrite("json", relativePath, data, delayMs);
}

export function scheduleTextWrite(relativePath: string, content: string, delayMs = 300): void {
    scheduleWrite("text", relativePath, content, delayMs);
}

export function schedulePathDelete(relativePath: string, delayMs = 0): void {
    scheduleWrite("delete", relativePath, null, delayMs);
}

export async function listPersistedDir(relativeDir = ""): Promise<Array<{ name: string; path: string; isDirectory: boolean }>> {
    return (await getBridge()?.listDataDir?.(relativeDir)) ?? [];
}

export async function openPersistedPath(relativePath: string): Promise<string> {
    return (await getBridge()?.openDataPath?.(relativePath)) ?? "Persistence bridge unavailable";
}

export async function revealPersistedPath(relativePath: string): Promise<boolean> {
    return (await getBridge()?.revealDataPath?.(relativePath)) ?? false;
}

export async function deletePersistedPath(relativePath: string): Promise<boolean> {
    return (await getBridge()?.deleteDataPath?.(relativePath)) ?? false;
}

export async function flushPendingWrites(): Promise<void> {
    const writes = [...pendingWrites.values()];
    for (const pending of writes) {
        if (pending.timer !== null && typeof window !== "undefined") {
            window.clearTimeout(pending.timer);
            pending.timer = null;
        }
    }

    await Promise.all(writes.map((pending) => flushWrite(pending)));
}