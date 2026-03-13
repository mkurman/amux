import { create } from "zustand";
import {
    readPersistedJson,
    scheduleJsonWrite,
} from "./persistence";

export interface SshProfile {
    id: string;
    name: string;
    host: string;
    user: string;
    port: number;
    keyPath: string;
    remotePath: string;
    jumpHost: string;
    options: string;
}

interface FileManagerState {
    sshProfiles: SshProfile[];
    addSshProfile: (profile?: Partial<SshProfile>) => string;
    updateSshProfile: (id: string, patch: Partial<SshProfile>) => void;
    removeSshProfile: (id: string) => void;
    buildSshCommand: (id: string) => string | null;
    hydrateProfiles: (profiles: SshProfile[]) => void;
}

const FILE_NAME = "file-manager.json";

let profileCounter = 0;

function normalizeProfile(input: Partial<SshProfile>): SshProfile {
    const id = typeof input.id === "string" && input.id
        ? input.id
        : `ssh_${++profileCounter}`;

    return {
        id,
        name: typeof input.name === "string" && input.name ? input.name : "New SSH Profile",
        host: typeof input.host === "string" ? input.host : "",
        user: typeof input.user === "string" ? input.user : "",
        port: Number.isFinite(input.port) ? Number(input.port) : 22,
        keyPath: typeof input.keyPath === "string" ? input.keyPath : "",
        remotePath: typeof input.remotePath === "string" && input.remotePath ? input.remotePath : "~",
        jumpHost: typeof input.jumpHost === "string" ? input.jumpHost : "",
        options: typeof input.options === "string" ? input.options : "",
    };
}

function normalizeProfiles(input: unknown): SshProfile[] {
    if (!Array.isArray(input)) return [];
    const profiles = input.map((entry) => normalizeProfile((entry ?? {}) as Partial<SshProfile>));

    for (const profile of profiles) {
        const match = /^ssh_(\d+)$/.exec(profile.id);
        if (match) {
            profileCounter = Math.max(profileCounter, Number(match[1]));
        }
    }

    return profiles;
}

function persistProfiles(profiles: SshProfile[]) {
    const payload = { sshProfiles: profiles };
    scheduleJsonWrite(FILE_NAME, payload, 250);
}

const initialProfiles: SshProfile[] = [];

export const useFileManagerStore = create<FileManagerState>((set, get) => ({
    sshProfiles: initialProfiles,

    addSshProfile: (profile) => {
        const normalized = normalizeProfile(profile ?? {});
        set((state) => {
            const sshProfiles = [normalized, ...state.sshProfiles];
            persistProfiles(sshProfiles);
            return { sshProfiles };
        });
        return normalized.id;
    },

    updateSshProfile: (id, patch) => {
        set((state) => {
            const sshProfiles = state.sshProfiles.map((profile) =>
                profile.id === id ? normalizeProfile({ ...profile, ...patch, id }) : profile
            );
            persistProfiles(sshProfiles);
            return { sshProfiles };
        });
    },

    removeSshProfile: (id) => {
        set((state) => {
            const sshProfiles = state.sshProfiles.filter((profile) => profile.id !== id);
            persistProfiles(sshProfiles);
            return { sshProfiles };
        });
    },

    buildSshCommand: (id) => {
        const profile = get().sshProfiles.find((entry) => entry.id === id);
        if (!profile || !profile.host) return null;

        const parts = ["ssh"];
        if (profile.port && profile.port !== 22) {
            parts.push("-p", String(profile.port));
        }
        if (profile.keyPath) {
            parts.push("-i", `'${profile.keyPath.replace(/'/g, "'\\''")}'`);
        }
        if (profile.jumpHost) {
            parts.push("-J", profile.jumpHost);
        }
        if (profile.options.trim()) {
            parts.push(profile.options.trim());
        }

        const target = profile.user ? `${profile.user}@${profile.host}` : profile.host;
        if (profile.remotePath && profile.remotePath.trim()) {
            parts.push(target, `-t`, `'cd ${profile.remotePath.replace(/'/g, "'\\''")} && exec $SHELL -l'`);
        } else {
            parts.push(target);
        }

        return parts.join(" ");
    },

    hydrateProfiles: (profiles) => {
        const normalized = normalizeProfiles(profiles);
        persistProfiles(normalized);
        set({ sshProfiles: normalized });
    },
}));

export async function hydrateFileManagerStore(): Promise<void> {
    const diskState = await readPersistedJson<{ sshProfiles?: SshProfile[] }>(FILE_NAME);
    const diskProfiles = normalizeProfiles(diskState?.sshProfiles ?? []);
    if (diskProfiles.length > 0) {
        useFileManagerStore.getState().hydrateProfiles(diskProfiles);
        return;
    }

    if (initialProfiles.length > 0) {
        scheduleJsonWrite(FILE_NAME, { sshProfiles: initialProfiles }, 0);
    }
}
