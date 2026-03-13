export interface PluginAssistantToolDefinition {
    type: "function";
    function: {
        name: string;
        description: string;
        parameters: Record<string, unknown>;
    };
}

export interface PluginAssistantToolCall {
    id: string;
    type: "function";
    function: {
        name: string;
        arguments: string;
    };
}

export interface PluginAssistantToolResult {
    toolCallId: string;
    name: string;
    content: string;
}

export type PluginAssistantToolExecutor = (
    call: PluginAssistantToolCall,
    args: Record<string, unknown>,
) => Promise<PluginAssistantToolResult> | PluginAssistantToolResult;

type RegisteredPluginAssistantTool = {
    pluginId: string;
    tool: PluginAssistantToolDefinition;
    executor: PluginAssistantToolExecutor;
};

const registeredToolsByName = new Map<string, RegisteredPluginAssistantTool>();
const registrationsByToolName = new Map<string, RegisteredPluginAssistantTool[]>();
const registeredNamesByPlugin = new Map<string, string[]>();

export function registerPluginAssistantTools(
    pluginId: string,
    tools?: PluginAssistantToolDefinition[],
    executors?: Record<string, PluginAssistantToolExecutor>,
): void {
    if (!tools?.length) {
        return;
    }

    const addedNames: string[] = [];

    for (const tool of tools) {
        const name = tool.function.name.trim();
        if (!name) {
            continue;
        }

        const executor = executors?.[name];
        if (!executor) {
            console.warn(`Assistant tool '${name}' from plugin '${pluginId}' is missing an executor; skipping registration.`);
            continue;
        }

        const registration = {
            pluginId,
            tool,
            executor,
        };

        const existingRegistrations = registrationsByToolName.get(name) ?? [];
        const nextRegistrations = existingRegistrations.filter((entry) => entry.pluginId !== pluginId);
        if (existingRegistrations.length > 0 && !existingRegistrations.some((entry) => entry.pluginId === pluginId)) {
            console.warn(`Assistant tool '${name}' is already registered; keeping '${pluginId}' as a fallback registration.`);
        }

        nextRegistrations.push(registration);
        registrationsByToolName.set(name, nextRegistrations);
        registeredToolsByName.set(name, nextRegistrations[0]);
        addedNames.push(name);
    }

    if (addedNames.length > 0) {
        const previous = registeredNamesByPlugin.get(pluginId) ?? [];
        registeredNamesByPlugin.set(pluginId, Array.from(new Set([...previous, ...addedNames])));
    }
}

export function unregisterPluginAssistantTools(pluginId: string): void {
    const names = registeredNamesByPlugin.get(pluginId);
    if (!names?.length) {
        return;
    }

    for (const name of names) {
        const remaining = (registrationsByToolName.get(name) ?? []).filter((entry) => entry.pluginId !== pluginId);
        if (remaining.length > 0) {
            registrationsByToolName.set(name, remaining);
            registeredToolsByName.set(name, remaining[0]);
        } else {
            registrationsByToolName.delete(name);
            registeredToolsByName.delete(name);
        }
    }

    registeredNamesByPlugin.delete(pluginId);
}

export function listPluginAssistantTools(): PluginAssistantToolDefinition[] {
    return Array.from(registeredToolsByName.values(), (entry) => entry.tool);
}

export async function executePluginAssistantTool(
    call: PluginAssistantToolCall,
    args: Record<string, unknown>,
): Promise<PluginAssistantToolResult | null> {
    const registered = registeredToolsByName.get(call.function.name);
    if (!registered) {
        return null;
    }

    return registered.executor(call, args);
}