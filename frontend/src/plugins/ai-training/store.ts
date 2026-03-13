import { create } from "zustand";
import { allLeafIds } from "../../lib/bspTree";
import { useWorkspaceStore } from "../../lib/workspaceStore";
import { buildAITrainingInstallCommand, buildAITrainingLaunchCommand, getAITrainingLaunchMode } from "./definitions";
import { discoverAITrainingProfiles, sendCommandToPane } from "./bridge";
import type { AITrainingDiscoveryStatus, DiscoveredAITraining } from "./types";

type LaunchTarget = {
    workspaceId: string;
    surfaceId: string;
    paneId: string;
};

type AITrainingState = {
    profiles: DiscoveredAITraining[];
    status: AITrainingDiscoveryStatus;
    error: string | null;
    selectedProfileId: string | null;
    selectedWorkspaceId: string | null;
    selectedSurfaceId: string | null;
    selectedPaneId: string | null;
    selectedLaunchModeId: string | null;
    launchPrompt: string;
    launchState: "idle" | "launching" | "success" | "error";
    launchError: string | null;
    lastLaunchCommand: string | null;
    installState: "idle" | "installing" | "success" | "error";
    installError: string | null;
    lastInstallCommand: string | null;
    refreshProfiles: (workspaceId?: string | null) => Promise<void>;
    setSelectedProfileId: (profileId: string | null) => void;
    setSelectedWorkspaceId: (workspaceId: string | null) => void;
    setSelectedSurfaceId: (surfaceId: string | null) => void;
    setSelectedPaneId: (paneId: string | null) => void;
    setSelectedLaunchModeId: (modeId: string | null) => void;
    setLaunchPrompt: (launchPrompt: string) => void;
    syncTargetSelection: (workspaceId: string | null, surfaceId: string | null, paneId: string | null) => void;
    launchSelectedProfile: () => Promise<boolean>;
    installSelectedProfile: () => Promise<boolean>;
};

function resolveLaunchTarget(workspaceId: string | null, surfaceId: string | null, paneId: string | null): LaunchTarget | null {
    const store = useWorkspaceStore.getState();
    const workspace = (workspaceId
        ? store.workspaces.find((entry) => entry.id === workspaceId)
        : undefined) ?? store.activeWorkspace();
    if (!workspace) {
        return null;
    }

    const surface = (surfaceId
        ? workspace.surfaces.find((entry) => entry.id === surfaceId)
        : undefined) ?? workspace.surfaces.find((entry) => entry.id === workspace.activeSurfaceId) ?? workspace.surfaces[0];
    if (!surface) {
        return null;
    }

    const paneIds = allLeafIds(surface.layout);
    const targetPaneId = paneId && paneIds.includes(paneId)
        ? paneId
        : surface.activePaneId && paneIds.includes(surface.activePaneId)
            ? surface.activePaneId
            : paneIds[0] ?? null;
    if (!targetPaneId) {
        return null;
    }

    return {
        workspaceId: workspace.id,
        surfaceId: surface.id,
        paneId: targetPaneId,
    };
}

function resolveWorkspacePath(workspaceId: string | null): string | null {
    const store = useWorkspaceStore.getState();
    const workspace = (workspaceId ? store.workspaces.find((entry) => entry.id === workspaceId) : undefined) ?? store.activeWorkspace();
    const cwd = workspace?.cwd?.trim();
    return cwd ? cwd : null;
}

function pickDefaultProfile(profiles: DiscoveredAITraining[], currentId: string | null): string | null {
    if (currentId && profiles.some((profile) => profile.id === currentId)) {
        return currentId;
    }

    return profiles.find((profile) => profile.available)?.id ?? profiles[0]?.id ?? null;
}

function pickDefaultLaunchMode(profile: DiscoveredAITraining | null, currentModeId: string | null): string | null {
    if (!profile) {
        return null;
    }

    return getAITrainingLaunchMode(profile, currentModeId).id;
}

export const useAITrainingStore = create<AITrainingState>((set, get) => ({
    profiles: [],
    status: "idle",
    error: null,
    selectedProfileId: null,
    selectedWorkspaceId: null,
    selectedSurfaceId: null,
    selectedPaneId: null,
    selectedLaunchModeId: null,
    launchPrompt: "",
    launchState: "idle",
    launchError: null,
    lastLaunchCommand: null,
    installState: "idle",
    installError: null,
    lastInstallCommand: null,

    refreshProfiles: async (workspaceId) => {
        const effectiveWorkspaceId = workspaceId ?? get().selectedWorkspaceId ?? useWorkspaceStore.getState().activeWorkspaceId ?? null;
        const workspacePath = resolveWorkspacePath(effectiveWorkspaceId);
        set({ status: "loading", error: null, selectedWorkspaceId: effectiveWorkspaceId });

        try {
            const profiles = await discoverAITrainingProfiles(workspacePath);
            const nextSelectedProfileId = pickDefaultProfile(profiles, get().selectedProfileId);
            set((state) => ({
                profiles,
                status: "ready",
                error: null,
                selectedWorkspaceId: effectiveWorkspaceId,
                selectedProfileId: nextSelectedProfileId,
                selectedLaunchModeId: pickDefaultLaunchMode(
                    profiles.find((profile) => profile.id === nextSelectedProfileId) ?? null,
                    state.selectedLaunchModeId,
                ),
            }));
        } catch (error) {
            set({
                profiles: [],
                status: "error",
                error: error instanceof Error ? error.message : "Failed to discover AI Training profiles.",
            });
        }
    },

    setSelectedProfileId: (selectedProfileId) => set((state) => {
        const profile = state.profiles.find((entry) => entry.id === selectedProfileId) ?? null;
        return {
            selectedProfileId,
            selectedLaunchModeId: pickDefaultLaunchMode(profile, state.selectedLaunchModeId),
            launchError: null,
            installError: null,
            launchState: state.launchState === "success" ? "idle" : state.launchState,
            installState: state.installState === "success" ? "idle" : state.installState,
        };
    }),
    setSelectedWorkspaceId: (selectedWorkspaceId) => set({ selectedWorkspaceId, selectedSurfaceId: null, selectedPaneId: null, launchError: null, installError: null }),
    setSelectedSurfaceId: (selectedSurfaceId) => set({ selectedSurfaceId, selectedPaneId: null, launchError: null, installError: null }),
    setSelectedPaneId: (selectedPaneId) => set({ selectedPaneId, launchError: null, installError: null }),
    setSelectedLaunchModeId: (selectedLaunchModeId) => set({ selectedLaunchModeId, launchError: null, installError: null, launchState: "idle" }),
    setLaunchPrompt: (launchPrompt) => set({ launchPrompt, launchError: null, installError: null, launchState: "idle" }),

    syncTargetSelection: (workspaceId, surfaceId, paneId) => {
        const target = resolveLaunchTarget(
            get().selectedWorkspaceId ?? workspaceId,
            get().selectedSurfaceId ?? surfaceId,
            get().selectedPaneId ?? paneId,
        );
        if (!target) {
            return;
        }

        set((state) => ({
            selectedWorkspaceId: state.selectedWorkspaceId && state.selectedWorkspaceId === target.workspaceId ? state.selectedWorkspaceId : target.workspaceId,
            selectedSurfaceId: state.selectedSurfaceId && state.selectedSurfaceId === target.surfaceId ? state.selectedSurfaceId : target.surfaceId,
            selectedPaneId: state.selectedPaneId && state.selectedPaneId === target.paneId ? state.selectedPaneId : target.paneId,
        }));
    },

    launchSelectedProfile: async () => {
        const state = get();
        const selectedProfile = state.profiles.find((profile) => profile.id === state.selectedProfileId);
        if (!selectedProfile) {
            set({ launchState: "error", launchError: "Choose an AI Training profile before launching." });
            return false;
        }

        if (!selectedProfile.available) {
            set({ launchState: "error", launchError: `${selectedProfile.label} is missing one or more required tools.` });
            return false;
        }

        const target = resolveLaunchTarget(state.selectedWorkspaceId, state.selectedSurfaceId, state.selectedPaneId);
        if (!target) {
            set({ launchState: "error", launchError: "Choose a valid target workspace, surface, and pane." });
            return false;
        }

        const workspacePath = resolveWorkspacePath(target.workspaceId);
        const launchMode = getAITrainingLaunchMode(selectedProfile, state.selectedLaunchModeId);
        const prompt = state.launchPrompt.trim();
        if (launchMode.requiresPrompt && !prompt) {
            set({ launchState: "error", launchError: `Provide input before launching ${selectedProfile.label} in ${launchMode.label} mode.` });
            return false;
        }

        if (launchMode.requiresWorkspace && !workspacePath) {
            set({ launchState: "error", launchError: `Set a workspace cwd before launching ${selectedProfile.label}.` });
            return false;
        }

        const command = buildAITrainingLaunchCommand(selectedProfile, state.selectedLaunchModeId, state.launchPrompt, workspacePath);
        if (!command) {
            set({ launchState: "error", launchError: `No launch command is defined for ${selectedProfile.label}.` });
            return false;
        }

        set({ launchState: "launching", launchError: null, lastLaunchCommand: command });

        const workspaceStore = useWorkspaceStore.getState();
        workspaceStore.setActiveWorkspace(target.workspaceId);
        workspaceStore.setActiveSurface(target.surfaceId);
        workspaceStore.setActivePaneId(target.paneId);

        try {
            await sendCommandToPane(target.paneId, command);
            set({
                launchState: "success",
                launchError: null,
                selectedWorkspaceId: target.workspaceId,
                selectedSurfaceId: target.surfaceId,
                selectedPaneId: target.paneId,
            });
            return true;
        } catch (error) {
            set({
                launchState: "error",
                launchError: error instanceof Error ? error.message : `Failed to launch ${selectedProfile.label}.`,
            });
            return false;
        }
    },

    installSelectedProfile: async () => {
        const state = get();
        const selectedProfile = state.profiles.find((profile) => profile.id === state.selectedProfileId);
        if (!selectedProfile) {
            set({ installState: "error", installError: "Choose an AI Training profile before installing prerequisites." });
            return false;
        }

        const target = resolveLaunchTarget(state.selectedWorkspaceId, state.selectedSurfaceId, state.selectedPaneId);
        if (!target) {
            set({ installState: "error", installError: "Choose a valid target workspace, surface, and pane." });
            return false;
        }

        const workspacePath = resolveWorkspacePath(target.workspaceId);
        const command = buildAITrainingInstallCommand(selectedProfile, workspacePath);
        if (!command) {
            set({ installState: "error", installError: `No install command is defined for ${selectedProfile.label}.` });
            return false;
        }

        set({ installState: "installing", installError: null, lastInstallCommand: command });

        const workspaceStore = useWorkspaceStore.getState();
        workspaceStore.setActiveWorkspace(target.workspaceId);
        workspaceStore.setActiveSurface(target.surfaceId);
        workspaceStore.setActivePaneId(target.paneId);

        try {
            await sendCommandToPane(target.paneId, command);
            set({
                installState: "success",
                installError: null,
                selectedWorkspaceId: target.workspaceId,
                selectedSurfaceId: target.surfaceId,
                selectedPaneId: target.paneId,
            });
            return true;
        } catch (error) {
            set({
                installState: "error",
                installError: error instanceof Error ? error.message : `Failed to install prerequisites for ${selectedProfile.label}.`,
            });
            return false;
        }
    },
}));