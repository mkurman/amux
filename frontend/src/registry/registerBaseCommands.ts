import { saveSession } from "../lib/sessionPersistence";
import { rollbackViewToDefault, saveViewDocument } from "../lib/cduiLoader";
import { useViewBuilderStore } from "../lib/viewBuilderStore";
import { useWorkspaceStore } from "../lib/workspaceStore";
import { registerCommand } from "./commandRegistry";

const logCommand = (message: string, payload?: unknown): void => {
  console.info(`[CDUI] ${message}`, payload ?? "");
};

const withWorkspace = <T>(fn: (state: ReturnType<typeof useWorkspaceStore.getState>, payload?: unknown) => T) => {
  return (payload?: unknown): T => fn(useWorkspaceStore.getState(), payload);
};

export const registerBaseCommands = (): void => {
  registerCommand("workspace.save", () => {
    saveSession();
    logCommand("workspace.save executed");
    return { ok: true };
  });

  registerCommand("workspace.execute", (payload?: unknown) => {
    logCommand("workspace.execute triggered", payload);
    return { ok: true, payload };
  });

  registerCommand("workspace.create", withWorkspace((state) => state.createWorkspace()));
  registerCommand("surface.create", withWorkspace((state) => state.createSurface()));

  registerCommand("pane.splitHorizontal", withWorkspace((state) => state.splitActive("horizontal")));
  registerCommand("pane.splitVertical", withWorkspace((state) => state.splitActive("vertical")));
  registerCommand("pane.toggleZoom", withWorkspace((state) => state.toggleZoom()));

  registerCommand("view.toggleSidebar", withWorkspace((state) => state.toggleSidebar()));
  registerCommand("view.toggleSettings", withWorkspace((state) => state.toggleSettings()));
  registerCommand("view.toggleSearch", withWorkspace((state) => state.toggleSearch()));
  registerCommand("view.toggleFileManager", withWorkspace((state) => state.toggleFileManager()));
  registerCommand("view.toggleCommandPalette", withWorkspace((state) => state.toggleCommandPalette()));
  registerCommand("view.toggleCommandHistory", withWorkspace((state) => state.toggleCommandHistory()));
  registerCommand("view.toggleCommandLog", withWorkspace((state) => state.toggleCommandLog()));
  registerCommand("view.toggleSessionVault", withWorkspace((state) => state.toggleSessionVault()));
  registerCommand("view.toggleSystemMonitor", withWorkspace((state) => state.toggleSystemMonitor()));
  registerCommand("view.toggleCanvas", withWorkspace((state) => state.toggleCanvas()));
  registerCommand("view.toggleTimeTravel", withWorkspace((state) => state.toggleTimeTravel()));
  registerCommand("view.toggleMission", withWorkspace((state) => state.toggleAgentPanel()));
  registerCommand("view.toggleNotifications", withWorkspace((state) => state.toggleNotificationPanel()));
  registerCommand("view.toggleSnippets", withWorkspace((state) => state.toggleSnippetPicker()));
  registerCommand("view.toggleWebBrowser", withWorkspace((state) => state.toggleWebBrowser()));
  registerCommand("view.reloadCDUI", () => {
    window.dispatchEvent(new Event("tamux-cdui-views-reload"));
    window.dispatchEvent(new Event("amux-cdui-views-reload"));
    return { ok: true };
  });

  registerCommand("builder.editNode", (payload?: unknown) => {
    const builderStore = useViewBuilderStore.getState();
    const selection = payload as { viewId?: string; nodeId?: string; componentType?: string } | undefined;
    if (!selection?.viewId || !selection.nodeId || !selection.componentType) {
      return { ok: false, error: "builder.editNode requires viewId, nodeId, and componentType." };
    }

    builderStore.startEditing({
      viewId: selection.viewId,
      nodeId: selection.nodeId,
      componentType: selection.componentType,
    });
    return { ok: true };
  });

  registerCommand("builder.stopEditing", () => {
    useViewBuilderStore.getState().stopEditing();
    return { ok: true };
  });

  registerCommand("builder.saveView", async () => {
    const builderStore = useViewBuilderStore.getState();
    const activeViewId = builderStore.activeViewId;
    if (!activeViewId) {
      return { ok: false, error: "No active builder view." };
    }

    const draft = builderStore.draftDocuments[activeViewId];
    if (!draft) {
      return { ok: false, error: `No draft document found for '${activeViewId}'.` };
    }

    const persisted = await saveViewDocument(activeViewId, draft);
    if (persisted) {
      useViewBuilderStore.getState().replaceActiveViewDocument(persisted.document);
      window.dispatchEvent(new Event("tamux-cdui-views-reload"));
      window.dispatchEvent(new Event("amux-cdui-views-reload"));
      return { ok: true, viewId: activeViewId };
    }

    return { ok: false, error: `Failed to save '${activeViewId}'.` };
  });

  registerCommand("builder.discardView", () => {
    const builderStore = useViewBuilderStore.getState();
    if (!builderStore.activeViewId) {
      return { ok: false, error: "No active builder view." };
    }

    builderStore.discardActiveView();
    return { ok: true, viewId: builderStore.activeViewId };
  });

  registerCommand("builder.resetView", async () => {
    const builderStore = useViewBuilderStore.getState();
    const activeViewId = builderStore.activeViewId;
    if (!activeViewId) {
      return { ok: false, error: "No active builder view." };
    }

    const reset = await rollbackViewToDefault(activeViewId);
    if (reset) {
      builderStore.replaceActiveViewDocument(reset.document);
      window.dispatchEvent(new Event("tamux-cdui-views-reload"));
      window.dispatchEvent(new Event("amux-cdui-views-reload"));
      return { ok: true, viewId: activeViewId };
    }

    return { ok: false, error: `Failed to reset '${activeViewId}'.` };
  });

  registerCommand("builder.toggleSelectedEditable", () => {
    const changed = useViewBuilderStore.getState().toggleSelectedNodeEditable();
    return { ok: changed };
  });

  registerCommand("builder.moveSelectedUp", () => {
    const changed = useViewBuilderStore.getState().moveSelectedNode("up");
    return { ok: changed };
  });

  registerCommand("builder.moveSelectedDown", () => {
    const changed = useViewBuilderStore.getState().moveSelectedNode("down");
    return { ok: changed };
  });

  registerCommand("builder.insertChild", (payload?: unknown) => {
    const spec = (payload as { componentType?: string; blockId?: string; targetNodeId?: string } | undefined) ?? {};
    if (!spec.componentType && !spec.blockId) {
      return { ok: false, error: "builder.insertChild requires componentType or blockId." };
    }

    const changed = useViewBuilderStore.getState().insertChildIntoSelectedNode(spec);
    return { ok: changed, componentType: spec.componentType, blockId: spec.blockId, targetNodeId: spec.targetNodeId };
  });

  registerCommand("builder.duplicateSelectedNode", () => {
    const changed = useViewBuilderStore.getState().duplicateSelectedNode();
    return { ok: changed };
  });

  registerCommand("builder.deleteSelectedNode", () => {
    const changed = useViewBuilderStore.getState().deleteSelectedNode();
    return { ok: changed };
  });

  registerCommand("builder.patchSelectedStyle", (payload?: unknown) => {
    const patch = (payload as Record<string, unknown> | undefined) ?? {};
    const changed = useViewBuilderStore.getState().patchSelectedNodeStyle(patch);
    return { ok: changed };
  });

  registerCommand("builder.patchSelectedProps", (payload?: unknown) => {
    const patch = (payload as Record<string, unknown> | undefined) ?? {};
    const changed = useViewBuilderStore.getState().patchSelectedNodeProps(patch);
    return { ok: changed };
  });

  registerCommand("builder.promoteSelectedToBlock", () => {
    const changed = useViewBuilderStore.getState().promoteSelectedNodeToBlock();
    return { ok: changed };
  });

  registerCommand("builder.moveNodeToTarget", (payload?: unknown) => {
    const move = payload as { draggedNodeId?: string; targetNodeId?: string } | undefined;
    if (!move?.draggedNodeId || !move.targetNodeId) {
      return { ok: false, error: "builder.moveNodeToTarget requires draggedNodeId and targetNodeId." };
    }

    const changed = useViewBuilderStore.getState().moveNodeToTarget(move.draggedNodeId, move.targetNodeId);
    return { ok: changed };
  });

  registerCommand("settings.openAbout", withWorkspace((state) => {
    if (!state.settingsOpen) {
      state.toggleSettings();
    }
    window.setTimeout(() => {
      window.dispatchEvent(new CustomEvent("tamux-open-settings-tab", { detail: { tab: "about" } }));
      window.dispatchEvent(new CustomEvent("amux-open-settings-tab", { detail: { tab: "about" } }));
    }, 50);
    return { ok: true };
  }));
};
