# Building CDUI Components And Views With YAML

This guide covers the current CDUI path in tamux: React components are registered in the frontend registry, and views compose those components through YAML documents.

## What YAML Controls

Each CDUI view is a YAML document with a small schema:

- `schemaVersion`: optional document version.
- `title`: human-readable name.
- `when`: optional visibility flag expression.
- `blocks`: reusable layout fragments with defaults.
- `layout`: the root node.
- `fallback`: optional fallback tree.
- `builder`: optional metadata for the visual builder.

Each node in `layout` or `blocks.*.layout` must define exactly one of:

- `type`: render a registered React component.
- `use`: expand a reusable block.

Nodes can also define:

- `id`: stable node id.
- `props`: component props.
- `command`: command id to invoke.
- `children`: nested nodes.
- `builder`: editability, droppable state, resize hints, and related builder metadata.

The authoritative schema lives in [frontend/src/schemas/uiSchema.ts](../frontend/src/schemas/uiSchema.ts).

## Where Views Live

There are two active view sources:

1. Built-in views in [frontend/src/views](../frontend/src/views).
2. Persisted user and plugin views under `~/.tamux/views`.

At runtime, tamux loads bundled defaults from [frontend/src/views](../frontend/src/views), writes compact persisted copies to `~/.tamux/views`, and then prefers the persisted version on later loads. Plugin views are loaded from `~/.tamux/views/plugins`.

The loader for this flow is [frontend/src/lib/cduiLoader.ts](../frontend/src/lib/cduiLoader.ts).

## Built-In View Workflow

Use this path when the view is part of the core product.

1. Add or update a React component in [frontend/src/components](../frontend/src/components) or a nearby module.
2. Make sure the component is registered under the name you want to reference from YAML.
3. Add or update the YAML in [frontend/src/views](../frontend/src/views).
4. If it is a new top-level view, add its raw import and id mapping in [frontend/src/lib/cduiLoader.ts](../frontend/src/lib/cduiLoader.ts).
5. If it should always participate in the default stack, add the view id to `DEFAULT_STACK` in [frontend/src/lib/cduiLoader.ts](../frontend/src/lib/cduiLoader.ts).
6. If it should be embedded into another view, mount it with `ViewMount` from the parent YAML instead of forcing it into the global stack.
7. Validate with `cd frontend && npm run build`.

## Minimal YAML Example

```yaml
schemaVersion: 1
title: "Example Panel"
when: "examplePanelOpen"
blocks:
  frame:
    title: "Example Frame"
    defaults:
      style:
        display: "flex"
        flexDirection: "column"
        minHeight: 0
        height: "100%"
    layout:
      use: "amux-primitive-column"
layout:
  id: "example-root"
  use: "frame"
  children:
    - id: "example-header"
      type: "SectionHeader"
      props:
        title: "Example"
    - id: "example-body"
      type: "SystemMonitorPanel"
      props:
        visible: true
```

This example shows the two core mechanisms:

- `blocks` define reusable structure and defaults.
- `layout` composes concrete registered components by `type`.

## Reusable Blocks And Primitives

The CDUI loader merges built-in primitive blocks at runtime. Common primitives include:

- `amux-primitive-box`
- `amux-primitive-row`
- `amux-primitive-column`

These are the safest building blocks for most layout work because they already carry the baseline flex and sizing defaults expected by the builder.

When composing your own blocks:

- Put repeated shell/layout structure in `blocks`.
- Put product-specific logic in `type` components.
- Keep `id` values stable so builder edits and persisted layouts remain predictable.

## Registering A Component For YAML Use

YAML `type` values only work if the component name is registered in the component registry.

The registry API lives in [frontend/src/registry/componentRegistry.ts](../frontend/src/registry/componentRegistry.ts), and the core export surface is assembled through [frontend/src/components/BaseComponents.tsx](../frontend/src/components/BaseComponents.tsx).

Practical rule:

- If you want to reference `type: "MyPanel"` in YAML, make sure `MyPanel` is exported and registered under exactly that name.

If you rename a component but leave old YAMLs in the wild, keep a compatibility alias until those YAMLs are migrated.

## Mounting A View Inside Another View

Top-level stack membership is not the only way to surface a view. Many panels are mounted from another YAML via `ViewMount`:

```yaml
- id: "agent-chat-panel-mount"
  type: "ViewMount"
  props:
    targetViewId: "agent-chat-panel"
```

Use `ViewMount` when:

- the child view should inherit the parent stage or overlay bounds,
- the child should appear only inside a specific shell region,
- the child already has its own YAML and should stay independently editable.

## Visibility And Layout Rules

Some recurring rules matter enough to be explicit:

- If a wrapped region should expand vertically, set `flex: 1` and `minHeight: 0`.
- If a shell wrapper is expected to behave like a flex column, declare it clearly in `style`.
- Overlay panels such as search and time-travel should usually mount inside the workspace frame, not as unrelated top-level absolute layers.
- Agent/browser side panels usually work best as siblings of the workspace frame inside the workspace stage.

Those patterns are already reflected in the shipped YAMLs such as [frontend/src/views/dashboard.yaml](../frontend/src/views/dashboard.yaml) and [frontend/src/views/agent-chat-panel.yaml](../frontend/src/views/agent-chat-panel.yaml).

## Builder Metadata

Builder metadata is optional at runtime but important if the visual editor should understand the node:

- `editable`: node can be selected and edited.
- `droppable`: node can accept child drops.
- `resizable`: node exposes resize handles.
- `movable`: node can be repositioned.
- `align`, `slot`, `data`: additional builder hints.

At block level, `builder.category` is useful for grouping reusable building blocks in the editor.

## Creating A New Top-Level View

Use a new top-level view only when the panel or surface has its own lifecycle and should load independently.

Checklist:

1. Create `frontend/src/views/my-view.yaml`.
2. Import it as raw text in [frontend/src/lib/cduiLoader.ts](../frontend/src/lib/cduiLoader.ts).
3. Add it to `DEFAULT_VIEW_YAMLS`.
4. Add it to `DEFAULT_STACK` if it should load by default.
5. Mount it from another view with `ViewMount` if it needs a specific visual slot.
6. Add the required React component exports or aliases so every referenced `type` resolves.

## Validating A YAML View

Use this loop:

1. Run `cd frontend && npm run build`.
2. Start the UI with `cd frontend && npm run dev` or `npm run dev:electron`.
3. Open with `?cdui=1` if you need to force CDUI mode.
4. Trigger `Reload CDUI Views` from the command palette after changing persisted YAMLs.
5. If a component does not render, first check whether the `type` name actually exists in the component registry.

## Common Failure Modes

- `Unknown Component: X`: the YAML `type` name is not registered under that exact string.
- Collapsed or invisible content: the surrounding shell is missing `flex: 1` or `minHeight: 0`.
- View loads from the wrong version: the persisted YAML in `~/.tamux/views` is overriding the bundled default.
- Plugin view not appearing: the YAML was not persisted under `~/.tamux/views/plugins` or failed schema validation.

## When To Use A Plugin Instead

Use a core built-in view when the feature belongs to tamux itself.

Use a plugin when you want to ship a feature that contributes one or more of:

- custom components,
- commands,
- assistant tools,
- plugin-owned YAML views.

For that path, see [docs/plugin-development.md](./plugin-development.md).