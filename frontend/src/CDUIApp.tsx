import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AppContext, defaultAppState } from "./context/AppContext";
import { ViewBuilderOverlay } from "./components/ViewBuilderOverlay";
import { LoadingState } from "./components/LoadingState";
import {
  compileViewDocument,
  loadCDUIViewStack,
  rollbackViewToDefault,
  type LoadedCDUIView,
} from "./lib/cduiLoader";
import { useViewBuilderStore } from "./lib/viewBuilderStore";
import "./plugins/globalAPI";
import DynamicRenderer from "./renderers/DynamicRenderer";
import { ViewErrorBoundary } from "./renderers/ViewErrorBoundary";
import { registerBaseCommands } from "./registry/registerBaseCommands";
import { registerBaseComponents } from "./registry/registerBaseComponents";
import { isCDUIViewVisible, useCDUIVisibilityFlags } from "./lib/cduiVisibility";
import { registerAITrainingPlugin } from "./plugins/ai-training/registerPlugin";
import { registerCodingAgentsPlugin } from "./plugins/coding-agents/registerPlugin";

const EMBEDDED_VIEW_IDS = new Set([
  "search-overlay",
  "time-travel-slider",
  "web-browser-panel",
  "agent-chat-panel",
]);

const TAMUX_LOADING_MARK = [
  "  _                             ",
  " | |_ __ _ _ __ ___  _   ___  __",
  " | __/ _` | '_ ` _ \\| | | \\ \/ /",
  " | || (_| | | | | | | |_| |>  < ",
  "  \\__\\__,_|_| |_| |_|\\__,_/_/\\_\\",
  "",
  "  Terminal Agentic Multiplexer",
].join("\n");

const CDUI_PLUGIN_VIEWS_UPDATED_EVENTS = ["tamux-cdui-plugin-views-updated", "amux-cdui-plugin-views-updated"] as const;
const CDUI_VIEWS_RELOAD_EVENTS = ["tamux-cdui-views-reload", "amux-cdui-views-reload"] as const;

const CDUIApp = () => {
  const [views, setViews] = useState<LoadedCDUIView[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const pendingCrashHandling = useRef<Set<string>>(new Set());
  const isEditMode = useViewBuilderStore((state) => state.isEditMode);
  const activeViewId = useViewBuilderStore((state) => state.activeViewId);
  const draftDocuments = useViewBuilderStore((state) => state.draftDocuments);
  const syncLoadedViews = useViewBuilderStore((state) => state.syncLoadedViews);
  const workspaceFlags = useCDUIVisibilityFlags();

  const reloadViews = useCallback(async () => {
    try {
      const loadedViews = await loadCDUIViewStack();
      setViews(loadedViews);
      setError(null);
    } catch (loadError) {
      console.error("Failed to load CDUI view stack", loadError);
      setError("Failed to load UI view stack");
    }
  }, []);

  useEffect(() => {
    let cancelled = false;

    const initializeRuntime = async () => {
      registerBaseComponents();
      registerBaseCommands();

      try {
        const pluginResults = await (window.tamux ?? window.amux)?.loadInstalledPlugins?.();
        if (Array.isArray(pluginResults)) {
          const failedOrSkipped = pluginResults.filter(
            (result) => result && (result.status === "error" || result.status === "skipped"),
          );
          if (failedOrSkipped.length > 0) {
            console.warn("Some installed plugins failed to load", failedOrSkipped);
          }
        }
      } catch (pluginLoadError) {
        console.error("Failed to load installed plugins", pluginLoadError);
      }

      registerCodingAgentsPlugin();
      registerAITrainingPlugin();

      if (!cancelled) {
        await reloadViews();
      }
    };

    void initializeRuntime();

    return () => {
      cancelled = true;
    };
  }, [reloadViews]);

  useEffect(() => {
    if (views) {
      syncLoadedViews(views);
    }
  }, [syncLoadedViews, views]);

  useEffect(() => {
    // Collapse multiple same-tick events into a single reloadViews() invocation
    let reloadScheduled = false;

    const scheduleReload = () => {
      if (reloadScheduled) {
        return;
      }
      reloadScheduled = true;
      window.setTimeout(() => {
        reloadScheduled = false;
        void reloadViews();
      }, 0);
    };

    const onPluginViewsUpdated = () => {
      scheduleReload();
    };
    const onManualViewsReload = () => {
      scheduleReload();
    };

    CDUI_PLUGIN_VIEWS_UPDATED_EVENTS.forEach((eventName) => window.addEventListener(eventName, onPluginViewsUpdated));
    CDUI_VIEWS_RELOAD_EVENTS.forEach((eventName) => window.addEventListener(eventName, onManualViewsReload));
    return () => {
      CDUI_PLUGIN_VIEWS_UPDATED_EVENTS.forEach((eventName) => window.removeEventListener(eventName, onPluginViewsUpdated));
      CDUI_VIEWS_RELOAD_EVENTS.forEach((eventName) => window.removeEventListener(eventName, onManualViewsReload));
    };
  }, [reloadViews]);

  const handleViewCrash = useCallback((viewId: string, crashError: Error) => {
    if (pendingCrashHandling.current.has(viewId)) {
      return;
    }
    pendingCrashHandling.current.add(viewId);

    // Defer state updates outside the current error boundary commit cycle.
    window.setTimeout(() => {
      setViews((current) => {
        if (!current) {
          return current;
        }

        const crashingView = current.find((view) => view.id === viewId);
        if (!crashingView) {
          return current;
        }

        // If the default view itself crashes, remove it to break render loops.
        if (crashingView.source === "default") {
          console.error(`Default view '${viewId}' crashed and was disabled.`, crashError);
          return current.filter((view) => view.id !== viewId);
        }

        console.error(`View '${viewId}' crashed. Rolling back to default.`, crashError);
        void rollbackViewToDefault(viewId).then((fallbackView) => {
          setViews((next) => {
            if (!next) {
              return next;
            }

            if (!fallbackView) {
              return next.filter((view) => view.id !== viewId);
            }

            return next.map((view) => (view.id === viewId ? fallbackView : view));
          });
        });

        return current;
      });

      pendingCrashHandling.current.delete(viewId);
    }, 0);
  }, []);

  const renderedViews = useMemo(() => {
    if (!views) {
      return null;
    }

    return views.map((view) => {
      if (!isEditMode || activeViewId !== view.id) {
        return view;
      }

      const draftDocument = draftDocuments[view.id];
      if (!draftDocument) {
        return view;
      }

      try {
        return {
          ...view,
          document: draftDocument,
          config: compileViewDocument(draftDocument, `builder:${view.id}`),
        } satisfies LoadedCDUIView;
      } catch (draftError) {
        console.warn(`Failed to compile builder draft for '${view.id}'.`, draftError);
        return view;
      }
    });
  }, [activeViewId, draftDocuments, isEditMode, views]);

  if (error) {
    return <div style={{ color: "red", padding: 16 }}>{error}</div>;
  }

  if (!views || !renderedViews) {
    return (
      <div
        style={{
          minHeight: "100vh",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          padding: 24,
          background: "radial-gradient(circle at top, rgba(34, 211, 238, 0.08), transparent 34%), var(--bg-primary)",
        }}
      >
        <div
          style={{
            display: "grid",
            justifyItems: "center",
            gap: 20,
            width: "min(100%, 820px)",
          }}
        >
          <pre
            style={{
              margin: 0,
              padding: "24px 28px",
              maxWidth: "100%",
              overflowX: "auto",
              borderRadius: "var(--radius-xl)",
              border: "1px solid var(--glass-border)",
              background: "linear-gradient(180deg, rgba(15, 21, 32, 0.96), rgba(10, 15, 24, 0.98))",
              boxShadow: "0 24px 80px rgba(0, 0, 0, 0.36)",
              color: "var(--text-primary)",
              fontFamily: "var(--font-mono)",
              fontSize: "clamp(10px, 1.25vw, 14px)",
              lineHeight: 1.3,
              textAlign: "left",
            }}
          >
            {TAMUX_LOADING_MARK}
          </pre>

          <div style={{ display: "grid", justifyItems: "center", gap: 8 }}>
            <LoadingState variant="spinner" size={22} />
            <div
              style={{
                fontSize: "var(--text-sm)",
                letterSpacing: "0.08em",
                textTransform: "uppercase",
                color: "var(--text-secondary)",
              }}
            >
              Loading CDUI...
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <AppContext.Provider value={defaultAppState}>
      {renderedViews.map((view) => {
        if (EMBEDDED_VIEW_IDS.has(view.id)) {
          return null;
        }

        if (!isCDUIViewVisible(workspaceFlags, view.id, view.config.when)) {
          return null;
        }

        return (
          <ViewErrorBoundary
            key={`${view.id}:${view.resetKey}`}
            viewId={view.id}
            resetKey={view.resetKey}
            onCrash={handleViewCrash}
          >
            <DynamicRenderer viewId={view.id} config={view.config.layout} fallback={view.config.fallback} />
          </ViewErrorBoundary>
        );
      })}
      <ViewBuilderOverlay />
    </AppContext.Provider>
  );
};

export default CDUIApp;
