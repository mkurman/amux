import { readPersistedJson, scheduleJsonWrite } from "./persistence";

const CDUI_PREFERENCE_FILE = "preferences/cdui.json";

let cachedPreference: boolean | null = null;

const readStoredCDUIPreference = (): boolean | null => cachedPreference;

export const hydrateCDUIPreference = async (): Promise<void> => {
  const persisted = await readPersistedJson<{ enabled?: boolean } | boolean>(CDUI_PREFERENCE_FILE);
  if (typeof persisted === "boolean") {
    cachedPreference = persisted;
    return;
  }

  if (persisted && typeof persisted === "object" && typeof persisted.enabled === "boolean") {
    cachedPreference = persisted.enabled;
  }
};

const storeCDUIPreference = (enabled: boolean): void => {
  cachedPreference = enabled;
  scheduleJsonWrite(CDUI_PREFERENCE_FILE, { enabled }, 0);
};

const readQueryOverride = (): boolean | null => {
  const query = new URLSearchParams(window.location.search);

  if (query.get("cdui") === "1" || query.get("ui") === "cdui") {
    return true;
  }

  if (query.get("cdui") === "0" || query.get("ui") === "classic") {
    return false;
  }

  return null;
};

export const isCDUIEnabled = (): boolean => {
  const queryOverride = readQueryOverride();
  if (queryOverride != null) {
    return queryOverride;
  }

  const storedPreference = readStoredCDUIPreference();
  if (storedPreference != null) {
    return storedPreference;
  }

  return true;
};

export const setCDUIEnabled = (enabled: boolean): void => {
  storeCDUIPreference(enabled);

  const url = new URL(window.location.href);
  url.searchParams.set("cdui", enabled ? "1" : "0");

  if (enabled) {
    if (url.searchParams.get("ui") === "classic") {
      url.searchParams.delete("ui");
    }
  } else {
    url.searchParams.set("ui", "classic");
  }

  window.history.replaceState({}, "", `${url.pathname}${url.search}${url.hash}`);
};
