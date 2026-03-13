import yaml from "js-yaml";
import { flushPendingWrites, scheduleTextWrite } from "../lib/persistence";
import { ComponentRegistryAPI } from "../registry/componentRegistry";
import { CommandRegistryAPI } from "../registry/commandRegistry";
import { PluginManager, type Plugin } from "./PluginManager";

const PLUGIN_VIEWS_DIR = "views/plugins";

const getPluginManager = (): PluginManager => {
  if (!window.__amuxPluginManager) {
    window.__amuxPluginManager = new PluginManager();
  }

  return window.__amuxPluginManager;
};

const persistPluginViews = async (plugin: Plugin): Promise<void> => {
  if (!plugin.views || Object.keys(plugin.views).length === 0) {
    return;
  }

  Object.entries(plugin.views).forEach(([viewName, viewValue]) => {
    const safeName = viewName.replace(/[^a-zA-Z0-9_-]/g, "-");
    const relativePath = `${PLUGIN_VIEWS_DIR}/${plugin.id}-${safeName}.yaml`;
    const content = typeof viewValue === "string" ? viewValue : yaml.dump(viewValue);
    scheduleTextWrite(relativePath, content, 0);
  });

  await flushPendingWrites();
};

export interface AmuxPluginAPI {
  registerComponent: typeof ComponentRegistryAPI.register;
  registerCommand: typeof CommandRegistryAPI.register;
  registerPlugin: (plugin: Plugin) => void;
  unregisterPlugin: (id: string) => void;
  getComponents: typeof ComponentRegistryAPI.list;
  getCommands: typeof CommandRegistryAPI.list;
  getPlugins: () => string[];
}

declare global {
  interface Window {
    AmuxApi?: AmuxPluginAPI;
    __amuxPluginManager?: PluginManager;
  }
}

const pluginManager = getPluginManager();

window.AmuxApi = {
  registerComponent: ComponentRegistryAPI.register,
  registerCommand: CommandRegistryAPI.register,
  registerPlugin: (plugin: Plugin) => {
    pluginManager.registerPlugin(plugin);
    void persistPluginViews(plugin).then(() => {
      window.dispatchEvent(new Event("amux-cdui-plugin-views-updated"));
    });
  },
  unregisterPlugin: (id: string) => {
    pluginManager.unregisterPlugin(id);
  },
  getComponents: ComponentRegistryAPI.list,
  getCommands: CommandRegistryAPI.list,
  getPlugins: () => pluginManager.listPlugins(),
};

export { pluginManager };
