import yaml from "js-yaml";
import { z } from "zod";
import {
  flushPendingWrites,
  listPersistedDir,
  readPersistedText,
  scheduleTextWrite,
} from "./persistence";
import {
  ViewDocumentSchema,
  type UIComponentNode,
  type UINodeBuilderMeta,
  type ViewConfig,
  type ViewDocument,
  type UIViewBlockDefinition,
  type UIViewNode,
} from "../schemas/uiSchema";
import { mergeViewBuilderPrimitiveBlocks, VIEW_BUILDER_PRIMITIVE_BLOCK_IDS } from "./viewBuilderPrimitives";

import stackYaml from "../views/stack.yaml?raw";
import dashboardYaml from "../views/dashboard.yaml?raw";
import commandPaletteYaml from "../views/command-palette.yaml?raw";
import notificationPanelYaml from "../views/notification-panel.yaml?raw";
import settingsPanelYaml from "../views/settings-panel.yaml?raw";
import sessionVaultPanelYaml from "../views/session-vault-panel.yaml?raw";
import commandLogPanelYaml from "../views/command-log-panel.yaml?raw";
import commandHistoryPickerYaml from "../views/command-history-picker.yaml?raw";
import searchOverlayYaml from "../views/search-overlay.yaml?raw";
import agentChatPanelYaml from "../views/agent-chat-panel.yaml?raw";
import snippetPickerYaml from "../views/snippet-picker.yaml?raw";
import systemMonitorPanelYaml from "../views/system-monitor-panel.yaml?raw";
import fileManagerPanelYaml from "../views/file-manager-panel.yaml?raw";
import timeTravelSliderYaml from "../views/time-travel-slider.yaml?raw";
import executionCanvasYaml from "../views/execution-canvas.yaml?raw";
import webBrowserPanelYaml from "../views/web-browser-panel.yaml?raw";
import agentApprovalOverlayYaml from "../views/agent-approval-overlay.yaml?raw";

const VIEWS_DIR = "views";
const STACK_PATH = `${VIEWS_DIR}/stack.yaml`;
const PLUGINS_DIR = `${VIEWS_DIR}/plugins`;
const DEFAULTS_VERSION_PATH = `${VIEWS_DIR}/.defaults-version`;
const DEFAULTS_VERSION = "13";

const DEFAULT_VIEW_YAMLS: Record<string, string> = {
  dashboard: dashboardYaml,
  "command-palette": commandPaletteYaml,
  "notification-panel": notificationPanelYaml,
  "settings-panel": settingsPanelYaml,
  "session-vault-panel": sessionVaultPanelYaml,
  "command-log-panel": commandLogPanelYaml,
  "command-history-picker": commandHistoryPickerYaml,
  "search-overlay": searchOverlayYaml,
  "agent-chat-panel": agentChatPanelYaml,
  "snippet-picker": snippetPickerYaml,
  "system-monitor-panel": systemMonitorPanelYaml,
  "file-manager-panel": fileManagerPanelYaml,
  "time-travel-slider": timeTravelSliderYaml,
  "execution-canvas": executionCanvasYaml,
  "web-browser-panel": webBrowserPanelYaml,
  "agent-approval-overlay": agentApprovalOverlayYaml,
};

const DEFAULT_STACK = [
  "dashboard",
  "search-overlay",
  "time-travel-slider",
  "web-browser-panel",
  "agent-chat-panel",
  "settings-panel",
  "session-vault-panel",
  "command-log-panel",
  "system-monitor-panel",
  "file-manager-panel",
  "command-palette",
  "notification-panel",
  "command-history-picker",
  "snippet-picker",
  "execution-canvas",
  "agent-approval-overlay",
] as const;

const StackSchema = z.object({
  views: z.array(z.string().min(1)).min(1),
});

export type ViewSource = "user" | "default" | "plugin";

export interface LoadedCDUIView {
  id: string;
  document: ViewDocument;
  config: ViewConfig;
  source: ViewSource;
  resetKey: string;
}

const viewPath = (viewId: string): string => `${VIEWS_DIR}/${viewId}.yaml`;

const makeResetKey = (viewId: string): string =>
  `${viewId}:${Date.now()}:${Math.random().toString(16).slice(2)}`;

const DEFAULT_VIEW_SCHEMA_VERSION = 1;

const slugify = (value: string): string => value
  .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
  .replace(/[^a-zA-Z0-9]+/g, "-")
  .replace(/^-+|-+$/g, "")
  .toLowerCase() || "view";

const makeRootBlockId = (node: UIViewNode): string => `view-${slugify(node.type ?? node.use ?? "root")}-root`;

const upgradeLegacyViewDocument = (document: ViewDocument): ViewDocument => {
  const baseDocument: ViewDocument = {
    ...document,
    schemaVersion: document.schemaVersion ?? DEFAULT_VIEW_SCHEMA_VERSION,
  };

  if (baseDocument.layout.use || baseDocument.blocks) {
    return baseDocument;
  }

  const blockId = makeRootBlockId(baseDocument.layout);

  return {
    ...baseDocument,
    blocks: {
      [blockId]: {
        title: `${baseDocument.title ?? baseDocument.layout.type ?? "View"} Root`,
        layout: baseDocument.layout,
        builder: {
          category: "view",
          editable: true,
        },
      },
    },
    layout: {
      id: baseDocument.layout.id ?? "view-root-instance",
      use: blockId,
      builder: {
        editable: true,
        droppable: true,
      },
    },
  };
};

const finalizeViewDocument = (document: ViewDocument): ViewDocument => {
  const upgraded = upgradeLegacyViewDocument(document);

  return materializeDocumentNodeIds({
    ...upgraded,
    schemaVersion: upgraded.schemaVersion ?? DEFAULT_VIEW_SCHEMA_VERSION,
    blocks: mergeViewBuilderPrimitiveBlocks(upgraded.blocks),
  });
};

const compactViewDocumentForPersistence = (document: ViewDocument): ViewDocument => {
  const blocks = document.blocks
    ? Object.fromEntries(
      Object.entries(document.blocks).filter(([key]) => !VIEW_BUILDER_PRIMITIVE_BLOCK_IDS.has(key)),
    )
    : undefined;

  return {
    ...document,
    ...(blocks && Object.keys(blocks).length > 0 ? { blocks } : {}),
    ...((!blocks || Object.keys(blocks).length === 0) ? { blocks: undefined } : {}),
  };
};

const hasRuntimePrimitiveBlocks = (document: ViewDocument): boolean =>
  Object.keys(document.blocks ?? {}).some((key) => VIEW_BUILDER_PRIMITIVE_BLOCK_IDS.has(key));

const buildPersistedDefaultYaml = (viewId: string): string => {
  const raw = DEFAULT_VIEW_YAMLS[viewId];
  const source = `default:${viewPath(viewId)}`;
  const parsed = yaml.load(raw);
  const result = ViewDocumentSchema.safeParse(parsed);

  if (!result.success) {
    throw new Error(`Invalid bundled default schema in ${source}: ${result.error.message}`);
  }

  return serializeViewDocument(compactViewDocumentForPersistence(finalizeViewDocument(result.data)));
};

const withNodeIds = (node: UIViewNode, trace: string[]): UIViewNode => ({
  ...node,
  id: node.id ?? trace.join("/"),
  ...(node.children
    ? {
      children: node.children.map((child, index) =>
        withNodeIds(child, [...trace, node.type ?? node.use ?? "node", String(index)]),
      ),
    }
    : {}),
});

const materializeDocumentNodeIds = (document: ViewDocument): ViewDocument => ({
  ...document,
  layout: withNodeIds(document.layout, ["layout"]),
  ...(document.fallback ? { fallback: withNodeIds(document.fallback, ["fallback"]) } : {}),
  ...(document.blocks
    ? {
      blocks: Object.fromEntries(Object.entries(document.blocks).map(([key, block]) => [
        key,
        {
          ...block,
          layout: withNodeIds(block.layout, ["blocks", key, "layout"]),
        },
      ])),
    }
    : {}),
});

const isPlainObject = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === "object" && !Array.isArray(value);

const mergeNodeProps = (
  baseProps?: Record<string, unknown>,
  overrideProps?: Record<string, unknown>,
): Record<string, unknown> | undefined => {
  if (!baseProps && !overrideProps) {
    return undefined;
  }

  const mergedProps: Record<string, unknown> = {
    ...(baseProps ?? {}),
    ...(overrideProps ?? {}),
  };

  const baseStyle = isPlainObject(baseProps?.style) ? baseProps.style : undefined;
  const overrideStyle = isPlainObject(overrideProps?.style) ? overrideProps.style : undefined;
  if (baseStyle || overrideStyle) {
    mergedProps.style = {
      ...(baseStyle ?? {}),
      ...(overrideStyle ?? {}),
    };
  }

  return mergedProps;
};

const mergeBuilderMeta = (
  baseMeta: Partial<UINodeBuilderMeta> | undefined,
  overrideMeta: Partial<UINodeBuilderMeta> | undefined,
): UINodeBuilderMeta | undefined => {
  if (!baseMeta && !overrideMeta) {
    return undefined;
  }

  const data = mergeNodeProps(baseMeta?.data, overrideMeta?.data);

  return {
    ...(baseMeta ?? {}),
    ...(overrideMeta ?? {}),
    ...(data ? { data } : {}),
  };
};

const applyBuilderRuntimePropFallbacks = (
  props: Record<string, unknown> | undefined,
  builder: Partial<UINodeBuilderMeta> | undefined,
): Record<string, unknown> | undefined => {
  if (!props && builder?.resizable === undefined && builder?.resizeAxis === undefined) {
    return undefined;
  }

  return {
    ...(props ?? {}),
    ...((props?.resizable === undefined && builder?.resizable !== undefined)
      ? { resizable: builder.resizable }
      : {}),
    ...((props?.resizeAxis === undefined && builder?.resizeAxis !== undefined)
      ? { resizeAxis: builder.resizeAxis }
      : {}),
  };
};

const normalizeViewNode = (
  node: UIViewNode,
  blocks: Record<string, UIViewBlockDefinition> | undefined,
  source: string,
  trace: string[],
): UIComponentNode => {
  const nodeId = node.id ?? trace.join("/");

  if (node.use) {
    const definition = blocks?.[node.use];
    if (!definition) {
      throw new Error(`Unknown block reference '${node.use}' in ${source}.`);
    }

    if (trace.includes(node.use)) {
      throw new Error(`Circular block reference '${node.use}' in ${source}.`);
    }

    const resolved = normalizeViewNode(definition.layout, blocks, source, [...trace, node.use]);
    const mergedChildren = node.children?.map((child, index) =>
      normalizeViewNode(child, blocks, source, [...trace, `${node.use}:child:${index}`]),
    ) ?? resolved.children;
    const builder = mergeBuilderMeta(
      definition.builder
        ? {
          editable: definition.builder.editable,
          data: definition.builder.data,
        }
        : undefined,
      {
        ...node.builder,
        editable: node.builder?.editable ?? definition.builder?.editable ?? true,
      },
    );

    return {
      nodeId,
      type: resolved.type,
      command: node.command ?? resolved.command,
      props: applyBuilderRuntimePropFallbacks(
        mergeNodeProps(
          mergeNodeProps(resolved.props, definition.defaults),
          node.props,
        ),
        node.builder,
      ),
      ...(builder ? { builder } : {}),
      ...(mergedChildren ? { children: mergedChildren } : {}),
    };
  }

  if (!node.type) {
    throw new Error(`Invalid node in ${source}: missing 'type'.`);
  }

  const children = node.children?.map((child, index) =>
    normalizeViewNode(child, blocks, source, [...trace, `${node.type}:${index}`]),
  );
  const builder = mergeBuilderMeta(undefined, {
    ...node.builder,
    editable: node.builder?.editable ?? Boolean(children?.length),
  });

  return {
    nodeId,
    type: node.type,
    ...(applyBuilderRuntimePropFallbacks(node.props, node.builder)
      ? { props: applyBuilderRuntimePropFallbacks(node.props, node.builder) }
      : {}),
    ...(node.command ? { command: node.command } : {}),
    ...(children ? { children } : {}),
    ...(builder ? { builder } : {}),
  };
};

const normalizeViewDocument = (document: ViewDocument, source: string): ViewConfig => ({
  schemaVersion: document.schemaVersion ?? DEFAULT_VIEW_SCHEMA_VERSION,
  title: document.title,
  when: document.when,
  layout: normalizeViewNode(document.layout, document.blocks, source, ["layout"]),
  ...(document.fallback
    ? { fallback: normalizeViewNode(document.fallback, document.blocks, source, ["fallback"]) }
    : {}),
});

export const compileViewDocument = (document: ViewDocument, source: string): ViewConfig =>
  normalizeViewDocument(document, source);

const parseViewDocument = (raw: string, source: string): ViewDocument => {
  const parsed = yaml.load(raw);
  const result = ViewDocumentSchema.safeParse(parsed);

  if (!result.success) {
    throw new Error(`Invalid view schema in ${source}: ${result.error.message}`);
  }

  return finalizeViewDocument(result.data);
};

export const serializeViewDocument = (document: ViewDocument): string => yaml.dump(document, {
  noRefs: true,
  lineWidth: 120,
  quotingType: '"',
  forceQuotes: false,
});

const parseViewStack = (raw: string, source: string): string[] => {
  const parsed = yaml.load(raw);
  const result = StackSchema.safeParse(parsed);

  if (!result.success) {
    throw new Error(`Invalid stack schema in ${source}: ${result.error.message}`);
  }

  return result.data.views;
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === "object" && !Array.isArray(value);

const isLikelyPlaceholderView = (raw: string): boolean => {
  try {
    const parsed = yaml.load(raw);
    if (!isRecord(parsed)) return false;
    const layout = parsed.layout;
    if (!isRecord(layout)) return false;
    const props = layout.props;
    const hasEmptyProps = isRecord(props) && Object.keys(props).length === 0;
    const hasNoChildren = !Array.isArray(layout.children);
    const hasOnlyTypeAndProps = Object.keys(layout).every((key) => key === "type" || key === "props");
    return hasEmptyProps && hasNoChildren && hasOnlyTypeAndProps;
  } catch {
    return false;
  }
};

const isLegacyAgentChatPanelView = (raw: string): boolean => {
  try {
    const parsed = yaml.load(raw);
    if (!isRecord(parsed)) return false;
    const layout = parsed.layout;
    if (!isRecord(layout)) return false;

    if (layout.type === "Container") {
      const legacyChildren = Array.isArray(layout.children) ? layout.children : [];
      const hasAgentChatChild = legacyChildren.some((child) => isRecord(child) && child.type === "AgentChatPanel");

      if (hasAgentChatChild) {
        const props = isRecord(layout.props) ? layout.props : {};
        const style = isRecord(props.style) ? props.style : {};
        const position = style.position;
        const hasAnchoredSizing = style.inset !== undefined || style.top !== undefined || style.bottom !== undefined;
        const legacyWidth = style.width === 420 || style.width === "420";

        if (position === "relative" && legacyWidth && !hasAnchoredSizing) {
          return true;
        }
      }
    }

    const blocks = isRecord(parsed.blocks) ? parsed.blocks : {};
    const layoutUse = typeof layout.use === "string" ? layout.use : null;
    const layoutChildren = Array.isArray(layout.children) ? layout.children : [];

    const shellBlock = isRecord(blocks.dockedAgentPanelShell) ? blocks.dockedAgentPanelShell : null;
    const shellDefaults = shellBlock && isRecord(shellBlock.defaults) ? shellBlock.defaults : null;
    const shellStyle = shellDefaults && isRecord(shellDefaults.style) ? shellDefaults.style : null;

    const findNodeById = (node: unknown, nodeId: string): Record<string, unknown> | null => {
      if (!isRecord(node)) {
        return null;
      }

      if (node.id === nodeId) {
        return node;
      }

      const children = Array.isArray(node.children) ? node.children : [];
      for (const child of children) {
        const match = findNodeById(child, nodeId);
        if (match) {
          return match;
        }
      }

      return null;
    };

    const providerNode = layoutChildren[0];
    const providerProps = isRecord(providerNode) && isRecord(providerNode.props) ? providerNode.props : null;
    const providerStyle = providerProps && isRecord(providerProps.style) ? providerProps.style : null;
    const currentSurfaceNode = layout ? findNodeById(layout, "agent-chat-current-surface") : null;
    const currentSurfaceProps = currentSurfaceNode && isRecord(currentSurfaceNode.props) ? currentSurfaceNode.props : null;
    const currentSurfaceStyle = currentSurfaceProps && isRecord(currentSurfaceProps.style) ? currentSurfaceProps.style : null;

    if (parsed.schemaVersion !== 1) return false;
    if (layoutUse !== "dockedAgentPanelShell") return false;
    if (Object.keys(blocks).length !== 1 || !isRecord(blocks.dockedAgentPanelShell)) return false;
    if (layoutChildren.length !== 1) return false;

    const child = layoutChildren[0];
    if (!isRecord(child) || child.type !== "AgentChatPanel") return false;

    const childProps = isRecord(child.props) ? child.props : {};
    const childStyle = isRecord(childProps.componentStyle) ? childProps.componentStyle : {};

    return shellStyle?.display !== "flex"
      || shellStyle?.flexDirection !== "column"
      || shellStyle?.minHeight !== 0
      || providerStyle?.display !== "flex"
      || providerStyle?.flexDirection !== "column"
      || providerStyle?.height !== "100%"
      || providerStyle?.minHeight !== 0
      || currentSurfaceStyle?.display !== "flex"
      || currentSurfaceStyle?.flexDirection !== "column"
      || currentSurfaceStyle?.flex !== 1
      || currentSurfaceStyle?.height !== "100%"
      || currentSurfaceStyle?.minHeight !== 0
      || (childProps.visible === true
        && childProps.resizable === true
        && childProps.resizeAxis === "horizontal"
        && childProps.minWidth === 380
        && childProps.maxWidth === "80vw"
        && childStyle.width === 560
        && childStyle.minWidth === 380
        && childStyle.maxWidth === 820
        && childStyle.height === "100%");
  } catch {
    return false;
  }
};

const isLegacyCommandPaletteView = (raw: string): boolean => {
  try {
    const parsed = yaml.load(raw);
    if (!isRecord(parsed)) return false;
    const layout = parsed.layout;
    if (!isRecord(layout)) return false;
    if (layout.type !== "Container") return false;

    const children = Array.isArray(layout.children) ? layout.children : [];
    const hasCommandPaletteChild = children.some((child) => isRecord(child) && child.type === "CommandPalette");
    if (!hasCommandPaletteChild) return false;

    const props = isRecord(layout.props) ? layout.props : {};
    const style = isRecord(props.style) ? props.style : {};
    const position = style.position;
    const hasTransform = typeof style.transform === "string" && style.transform.length > 0;

    return position === "fixed" && hasTransform;
  } catch {
    return false;
  }
};

const isLegacyDashboardView = (raw: string): boolean => {
  try {
    const parsed = yaml.load(raw);
    if (!isRecord(parsed)) return false;

    const blocks = isRecord(parsed.blocks) ? parsed.blocks : {};
    const workspaceStage = isRecord(blocks.workspaceStage) ? blocks.workspaceStage : null;
    const defaults = workspaceStage && isRecord(workspaceStage.defaults) ? workspaceStage.defaults : null;
    const style = defaults && isRecord(defaults.style) ? defaults.style : null;

    const findNodeById = (node: unknown, nodeId: string): Record<string, unknown> | null => {
      if (!isRecord(node)) {
        return null;
      }

      if (node.id === nodeId) {
        return node;
      }

      const children = Array.isArray(node.children) ? node.children : [];
      for (const child of children) {
        const match = findNodeById(child, nodeId);
        if (match) {
          return match;
        }
      }

      return null;
    };

    const layout = isRecord(parsed.layout) ? parsed.layout : null;
    const layoutContainer = layout ? findNodeById(layout, "layout-container") : null;
    const layoutProps = layoutContainer && isRecord(layoutContainer.props) ? layoutContainer.props : null;
    const layoutStyle = layoutProps && isRecord(layoutProps.style) ? layoutProps.style : null;
    const sidebarNode = layout ? findNodeById(layout, "sidebar") : null;
    const sidebarProps = sidebarNode && isRecord(sidebarNode.props) ? sidebarNode.props : null;
    const sidebarStyle = sidebarProps && isRecord(sidebarProps.style) ? sidebarProps.style : null;

    return style?.alignItems !== "stretch"
      || layoutStyle?.display !== "flex"
      || layoutStyle?.flexDirection !== "column"
      || layoutStyle?.height !== "100%"
      || sidebarStyle?.display !== "flex"
      || sidebarStyle?.flexDirection !== "column"
      || sidebarStyle?.height !== "100%"
      || sidebarStyle?.alignSelf !== "stretch";
  } catch {
    return false;
  }
};

const writeDefaults = async (writes: Array<{ relativePath: string; content: string }>): Promise<void> => {
  if (writes.length === 0) {
    return;
  }

  writes.forEach((write) => scheduleTextWrite(write.relativePath, write.content, 0));
  await flushPendingWrites();
};

const ensureUserViewDefaults = async (): Promise<void> => {
  const missingWrites: Array<{ relativePath: string; content: string }> = [];
  const defaultsVersion = (await readPersistedText(DEFAULTS_VERSION_PATH))?.trim() ?? null;
  const shouldUpgradeDefaults = defaultsVersion !== DEFAULTS_VERSION;

  for (const [viewId] of Object.entries(DEFAULT_VIEW_YAMLS)) {
    const relativePath = viewPath(viewId);
    const persistedDefaultYaml = buildPersistedDefaultYaml(viewId);
    const userYaml = await readPersistedText(relativePath);
    if (userYaml === null) {
      missingWrites.push({ relativePath, content: persistedDefaultYaml });
      continue;
    }

    if (isLikelyPlaceholderView(userYaml)) {
      missingWrites.push({ relativePath, content: persistedDefaultYaml });
      continue;
    }

    if (viewId === "agent-chat-panel" && isLegacyAgentChatPanelView(userYaml)) {
      missingWrites.push({ relativePath, content: persistedDefaultYaml });
      continue;
    }

    if (viewId === "dashboard" && isLegacyDashboardView(userYaml)) {
      missingWrites.push({ relativePath, content: persistedDefaultYaml });
      continue;
    }

    if (viewId === "command-palette" && isLegacyCommandPaletteView(userYaml)) {
      missingWrites.push({ relativePath, content: persistedDefaultYaml });
    }
  }

  const userStack = await readPersistedText(STACK_PATH);
  if (userStack === null) {
    missingWrites.push({ relativePath: STACK_PATH, content: stackYaml });
  }

  if (shouldUpgradeDefaults) {
    missingWrites.push({ relativePath: DEFAULTS_VERSION_PATH, content: DEFAULTS_VERSION });
  }

  await writeDefaults(missingWrites);
};

const loadStack = async (): Promise<string[]> => {
  const userStack = await readPersistedText(STACK_PATH);

  if (!userStack) {
    return [...DEFAULT_STACK];
  }

  try {
    return parseViewStack(userStack, `user:${STACK_PATH}`);
  } catch (error) {
    console.warn(error);
    await writeDefaults([{ relativePath: STACK_PATH, content: stackYaml }]);
    return [...DEFAULT_STACK];
  }
};

const loadViewById = async (viewId: string): Promise<LoadedCDUIView | null> => {
  const defaultYaml = DEFAULT_VIEW_YAMLS[viewId];
  if (!defaultYaml) {
    console.warn(`No default YAML found for view '${viewId}'.`);
    return null;
  }

  const userPath = viewPath(viewId);
  const userYaml = await readPersistedText(userPath);

  if (userYaml) {
    try {
      const document = parseViewDocument(userYaml, `user:${userPath}`);
      if (hasRuntimePrimitiveBlocks(document)) {
        const compactYaml = serializeViewDocument(compactViewDocumentForPersistence(document));
        if (compactYaml.trim() !== userYaml.trim()) {
          await writeDefaults([{ relativePath: userPath, content: compactYaml }]);
        }
      }
      const config = normalizeViewDocument(document, `user:${userPath}`);
      return { id: viewId, document, config, source: "user", resetKey: makeResetKey(viewId) };
    } catch (error) {
      console.warn(error);
      await writeDefaults([{ relativePath: userPath, content: buildPersistedDefaultYaml(viewId) }]);
    }
  }

  const fallbackDocument = parseViewDocument(defaultYaml, `default:${userPath}`);
  const fallbackConfig = normalizeViewDocument(fallbackDocument, `default:${userPath}`);
  return { id: viewId, document: fallbackDocument, config: fallbackConfig, source: "default", resetKey: makeResetKey(viewId) };
};

const loadPluginViews = async (): Promise<LoadedCDUIView[]> => {
  const entries = await listPersistedDir(PLUGINS_DIR);
  const pluginFiles = entries
    .filter((entry) => !entry.isDirectory && /\.ya?ml$/i.test(entry.name))
    .sort((a, b) => a.name.localeCompare(b.name));

  const loaded: LoadedCDUIView[] = [];

  for (const file of pluginFiles) {
    const raw = await readPersistedText(file.path);
    if (!raw) {
      continue;
    }

    try {
      const document = parseViewDocument(raw, `plugin:${file.path}`);
      const config = normalizeViewDocument(document, `plugin:${file.path}`);
      const id = `plugin:${file.name.replace(/\.ya?ml$/i, "")}`;
      loaded.push({ id, document, config, source: "plugin", resetKey: makeResetKey(id) });
    } catch (error) {
      console.warn(`Skipping invalid plugin view '${file.path}'.`, error);
    }
  }

  return loaded;
};

export const loadCDUIViewStack = async (): Promise<LoadedCDUIView[]> => {
  await ensureUserViewDefaults();

  const stack = await loadStack();
  const baseViews: LoadedCDUIView[] = [];

  for (const viewId of stack) {
    const view = await loadViewById(viewId);
    if (view) {
      baseViews.push(view);
    }
  }

  const pluginViews = await loadPluginViews();
  return [...baseViews, ...pluginViews];
};

export const rollbackViewToDefault = async (viewId: string): Promise<LoadedCDUIView | null> => {
  const defaultYaml = DEFAULT_VIEW_YAMLS[viewId];
  if (!defaultYaml) {
    return null;
  }

  const relativePath = viewPath(viewId);
  await writeDefaults([{ relativePath, content: buildPersistedDefaultYaml(viewId) }]);

  const document = parseViewDocument(defaultYaml, `default:${relativePath}`);
  const config = normalizeViewDocument(document, `default:${relativePath}`);
  return {
    id: viewId,
    document,
    config,
    source: "default",
    resetKey: makeResetKey(viewId),
  };
};

export const saveViewDocument = async (viewId: string, document: ViewDocument): Promise<LoadedCDUIView | null> => {
  const relativePath = viewPath(viewId);
  const persistedDocument = finalizeViewDocument(document);
  await writeDefaults([{ relativePath, content: serializeViewDocument(compactViewDocumentForPersistence(persistedDocument)) }]);

  const config = normalizeViewDocument(persistedDocument, `user:${relativePath}`);

  return {
    id: viewId,
    document: persistedDocument,
    config,
    source: "user",
    resetKey: makeResetKey(viewId),
  };
};
