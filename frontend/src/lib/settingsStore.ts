import { create } from "zustand";
import { AmuxSettings, DEFAULT_SETTINGS, ShellProfile } from "./types";
import {
  readPersistedJson,
  scheduleJsonWrite,
} from "./persistence";

const SETTINGS_FILE = "settings.json";

type PersistedSettingsState = {
  settings?: Partial<AmuxSettings>;
  profiles?: ShellProfile[];
};

function persistState(state: { settings: AmuxSettings; profiles: ShellProfile[] }) {
  const payload = {
    settings: state.settings,
    profiles: state.profiles,
  };

  scheduleJsonWrite(SETTINGS_FILE, payload, 500);
}

export interface SettingsState {
  settings: AmuxSettings;
  profiles: ShellProfile[];

  updateSetting: <K extends keyof AmuxSettings>(
    key: K,
    value: AmuxSettings[K]
  ) => void;
  resetSettings: () => void;
  loadSettings: (s: Partial<AmuxSettings>) => void;

  addProfile: (profile: ShellProfile) => void;
  removeProfile: (id: string) => void;
  updateProfile: (id: string, updates: Partial<ShellProfile>) => void;
  setDefaultProfile: (id: string) => void;
  getDefaultProfile: () => ShellProfile | undefined;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: { ...DEFAULT_SETTINGS },
  profiles: [],

  updateSetting: (key, value) => {
    set((s) => {
      const nextState = {
        settings: { ...s.settings, [key]: value },
      };
      persistState({
        settings: nextState.settings,
        profiles: s.profiles,
      });
      return nextState;
    });
  },

  resetSettings: () =>
    set((s) => {
      const nextState = { settings: { ...DEFAULT_SETTINGS } };
      persistState({ settings: nextState.settings, profiles: s.profiles });
      return nextState;
    }),

  loadSettings: (s) => {
    set((state) => {
      const nextState = {
        settings: { ...state.settings, ...s },
      };
      persistState({
        settings: nextState.settings,
        profiles: state.profiles,
      });
      return nextState;
    });
  },

  addProfile: (profile) => {
    set((s) => {
      const profiles = [...s.profiles, profile];
      persistState({ settings: s.settings, profiles });
      return { profiles };
    });
  },

  removeProfile: (id) => {
    set((s) => {
      const profiles = s.profiles.filter((p) => p.id !== id);
      persistState({ settings: s.settings, profiles });
      return { profiles };
    });
  },

  updateProfile: (id, updates) => {
    set((s) => {
      const profiles = s.profiles.map((p) =>
        p.id === id ? { ...p, ...updates } : p
      );
      persistState({ settings: s.settings, profiles });
      return { profiles };
    });
  },

  setDefaultProfile: (id) => {
    set((s) => {
      const profiles = s.profiles.map((p) => ({
        ...p,
        isDefault: p.id === id,
      }));
      persistState({ settings: s.settings, profiles });
      return { profiles };
    });
  },

  getDefaultProfile: () => get().profiles.find((p) => p.isDefault),
}));

export async function hydrateSettingsStore(): Promise<void> {
  const diskState = await readPersistedJson<PersistedSettingsState>(SETTINGS_FILE);
  if (diskState) {
    useSettingsStore.setState({
      settings: { ...DEFAULT_SETTINGS, ...(diskState.settings ?? {}) },
      profiles: Array.isArray(diskState.profiles) ? diskState.profiles : [],
    });
  }
}
