# Creating Your Own tamux Plugin

tamux plugins are frontend-registered extensions. A plugin can contribute:

- React components
- command handlers
- YAML views
- assistant-callable tools and executors
- startup and teardown hooks

The core runtime is implemented in [frontend/src/plugins/PluginManager.ts](../frontend/src/plugins/PluginManager.ts) and [frontend/src/plugins/globalAPI.ts](../frontend/src/plugins/globalAPI.ts).

## Current Plugin Models

tamux now supports two plugin paths.

### 1. In-Tree Plugins

This is the model used by the built-in coding-agents plugin.

1. Add plugin source code under [frontend/src/plugins](../frontend/src/plugins).
2. Import and register it from app startup code such as [frontend/src/CDUIApp.tsx](../frontend/src/CDUIApp.tsx).
3. Build and validate with the normal frontend or Electron workflows.

### 2. Runtime-Installed npm Plugins

You can now install a packaged plugin with:

```bash
tamux install plugin <npm-package>
```

You can also install from a local package directory:

```bash
tamux install plugin ../my-tamux-plugin
```

This packaged path is a distribution layer on top of the same runtime plugin contract described here.

Practical implication:

- Write plugins against the current runtime API.
- Package them as self-contained browser scripts for external installation.

## Plugin Interface

The runtime plugin shape is:

```ts
export interface Plugin {
  id: string;
  name: string;
  version: string;
  components?: Record<string, React.ComponentType<any>>;
  commands?: Record<string, CommandAction>;
  views?: Record<string, unknown>;
  assistantTools?: PluginAssistantToolDefinition[];
  assistantToolExecutors?: Record<string, PluginAssistantToolExecutor>;
  onLoad?: () => void;
  onUnload?: () => void;
}
```

Important behavior:

- component names are registered as `${plugin.id}:${name}`,
- command ids are registered as `${plugin.id}:${name}`,
- plugin YAML views are persisted into `views/plugins`,
- assistant tools are merged into the agent tool list at runtime.

## package.json Contract For npm Plugins

Externally installed packages should expose a `tamuxPlugin` field in `package.json`. The legacy `amuxPlugin` field is still accepted for compatibility.

Minimal example:

```json
{
  "name": "tamux-plugin-example",
  "version": "0.1.3",
  "tamuxPlugin": {
    "entry": "dist/tamux-plugin.js",
    "format": "script"
  }
}
```

Current rules:

- `entry` is required.
- only `format: "script"` is currently supported.
- the entry file must be self-contained and executable in the renderer without further bundling.
- plugin installers are run with `npm install --ignore-scripts`, so published packages must already contain their built entry assets.
- the script should register itself through `window.TamuxApi.registerPlugin(...)`. `window.AmuxApi.registerPlugin(...)` remains available as a compatibility alias.

Installed package metadata is recorded under `~/.tamux/plugins/registry.json`, and Electron preload loads those entries on app startup.

## Minimal Plugin Example

```ts
import type { Plugin } from "../PluginManager";
import ExamplePanel from "./ExamplePanel";

export const examplePlugin: Plugin = {
  id: "example",
  name: "Example Plugin",
  version: "0.1.3",
  components: {
    ExamplePanel,
  },
  commands: {
    sayHello: () => {
      console.info("hello from example plugin");
    },
  },
  views: {
    example: {
      schemaVersion: 1,
      title: "Example Plugin View",
      layout: {
        id: "example-root",
        type: "example:ExamplePanel",
      },
    },
  },
  onLoad: () => {
    console.info("example plugin loaded");
  },
};

export function registerExamplePlugin(): void {
  if (typeof window === "undefined" || !(window.TamuxApi || window.AmuxApi)) {
    return;
  }

  (window.TamuxApi ?? window.AmuxApi).registerPlugin(examplePlugin);
}
```

For an externally installed plugin, bundle that registration path into the script referenced by `tamuxPlugin.entry` so the script self-registers when loaded.

## Registering The Plugin

The plugin must be registered during app startup.

Pattern:

1. Create a `registerMyPlugin()` function in your plugin module.
2. Guard against duplicate registration.
3. Call `window.TamuxApi.registerPlugin(...)`.
4. Import and invoke the registration function from app initialization.

The coding-agents implementation in [frontend/src/plugins/coding-agents/registerPlugin.ts](../frontend/src/plugins/coding-agents/registerPlugin.ts) is the best current reference.

## Plugin Components

Use `components` when YAML or other runtime code needs to render a custom React component.

Notes:

- The plugin manager namespaces each component as `${plugin.id}:${name}`.
- In plugin YAML, reference the namespaced type string.
- Keep the component self-contained and avoid hidden cross-module side effects.

For npm-installed plugins, component code must be bundled into the external script in a way that can execute directly in the renderer.

Example YAML node:

```yaml
layout:
  id: "example-root"
  type: "example:ExamplePanel"
```

## Plugin Commands

Use `commands` for explicit UI actions that can be triggered by buttons, command palette entries, or YAML node commands.

Command ids are also namespaced as `${plugin.id}:${name}`.

Example:

```yaml
- id: "refresh-button"
  type: "Button"
  command: "example:refresh"
  props:
    label: "Refresh"
```

## Plugin Views

Plugins can ship YAML view documents through the `views` field.

Runtime behavior:

- each view entry is serialized to YAML,
- stored under `~/.tamux/views/plugins`,
- loaded by the CDUI loader after the base stack,
- assigned an id of the form `plugin:<filename-without-extension>`.

Use plugin views when the feature owns a complete panel, overlay, or embedded surface.

If you only need a small UI fragment inside an existing core view, adding a component and mounting it from existing YAML is often simpler.

## Assistant Tools In Plugins

Plugins can extend the assistant tool list with OpenAI-style function tools.

Use:

- `assistantTools` to declare schemas.
- `assistantToolExecutors` to execute them.

Each executor returns a tool result payload that the assistant runtime feeds back into the next model round.

This is the right extension point when your plugin should be directly invocable by the built-in assistant.

The coding-agents plugin uses this path to expose discovery and launch operations.

The built-in coding-agents plugin now also demonstrates a richer runtime-profile pattern for external tools such as Hermes, pi.dev, and OpenClaw: discovery is not limited to PATH presence, and can include config-path checks, setup guidance, launch modes, and local runtime health checks surfaced through Electron IPC.

The new built-in `ai-training` plugin follows the same renderer/preload/main split, but uses a separate domain model because training integrations are not all plain CLIs. Prime Intellect Verifiers behaves like a training runtime, while AutoResearch and AutoRL are repository-bound workflows that require workspace path and file-shape checks in addition to global system prerequisites.

## Electron Bridges For System-Level Plugins

If a plugin needs filesystem access, PATH discovery, process spawning, or other host capabilities, do not reach straight from React into Node APIs.

Use the Electron bridge layers:

- [frontend/electron/preload.cjs](../frontend/electron/preload.cjs)
- [frontend/electron/main.cjs](../frontend/electron/main.cjs)

Recommended split:

- renderer plugin code owns UI and state,
- preload exposes a small safe bridge,
- main process performs privileged operations.

Use `npm run dev:electron` for this category of plugin work.

## Suggested Plugin Layout

One practical layout is:

```text
frontend/src/plugins/my-plugin/
  registerPlugin.ts
  types.ts
  store.ts
  bridge.ts
  MyPluginView.tsx
```

This keeps the registration surface small while leaving room for feature-specific state and Electron bridge logic.

## Build And Validation

Use the smallest validation loop that matches the plugin surface area:

- UI-only plugin: `cd frontend && npm run build`
- plugin with Electron bridge work: `cd frontend && npm run build` and then `cd frontend && npm run dev:electron`
- lint pass if you touched broader frontend code: `cd frontend && npm run lint`

For an npm-distributed plugin package, also validate:

- `npm pack` produces the expected bundle,
- the published package contains the file referenced by `tamuxPlugin.entry` or the legacy `amuxPlugin.entry`,
- `tamux install plugin <path-or-package>` records the plugin in `~/.tamux/plugins/registry.json`,
- Electron startup loads the script without console errors.

For YAML-backed plugin views, also verify that:

- the persisted YAML appears under `~/.tamux/views/plugins`,
- the plugin view renders in CDUI mode,
- every referenced component type resolves.

## Common Failure Modes

- Plugin registers twice: guard registration with a module-level `registered` flag.
- YAML view persists but does not render: the component type is not namespaced correctly.
- Electron feature silently fails in browser preview: the bridge is only available in Electron.
- Assistant tool loops or repeats: ensure the executor returns a proper tool result and verify the tool name matches the declared schema.

## Design Guidance

- Keep plugin ids short and stable because they become part of public component and command names.
- Prefer explicit bridges over hidden globals for privileged behavior.
- Avoid coupling plugin code directly to daemon internals unless the feature truly needs a new backend capability.
- Treat plugin YAML as composition and layout, not as a place to hide business logic.

## Relationship To CDUI YAML Authoring

Plugin views use the same YAML view schema as core views. If you need the document structure, block pattern, or `ViewMount` conventions, see [docs/cdui-yaml-views.md](./cdui-yaml-views.md).