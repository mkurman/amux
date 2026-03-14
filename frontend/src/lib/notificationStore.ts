import { create } from "zustand";
import {
  TerminalNotification,
  NotificationId,
  NotificationSource,
  WorkspaceId,
  SurfaceId,
  PaneId,
} from "./types";

const MAX_NOTIFICATIONS = 500;

let _nId = 0;
function newNotifId(): NotificationId {
  return `notif_${++_nId}`;
}

export interface NotificationState {
  notifications: TerminalNotification[];
  unreadCount: number;

  addNotification: (opts: {
    title: string;
    body: string;
    subtitle?: string | null;
    icon?: string | null;
    progress?: number | null;
    source: NotificationSource;
    workspaceId?: WorkspaceId | null;
    surfaceId?: SurfaceId | null;
    paneId?: PaneId | null;
    panelId?: PaneId | null;
  }) => void;

  markRead: (id: NotificationId) => void;
  removeNotification: (id: NotificationId) => void;
  clearPaneNotifications: (paneId: PaneId, source?: NotificationSource) => void;
  markAllRead: () => void;
  markWorkspaceRead: (workspaceId: WorkspaceId) => void;
  clearAll: () => void;

  getUnreadForWorkspace: (workspaceId: WorkspaceId) => number;
}

export const useNotificationStore = create<NotificationState>((set, get) => ({
  notifications: [],
  unreadCount: 0,

  addNotification: (opts) => {
    const notif: TerminalNotification = {
      id: newNotifId(),
      workspaceId: opts.workspaceId ?? null,
      surfaceId: opts.surfaceId ?? null,
      paneId: opts.paneId ?? null,
      panelId: opts.panelId ?? opts.paneId ?? null,
      title: opts.title,
      subtitle: opts.subtitle ?? null,
      body: opts.body,
      icon: opts.icon ?? null,
      progress: opts.progress ?? null,
      isRead: false,
      timestamp: Date.now(),
      source: opts.source,
    };

    set((s) => {
      const updated = [notif, ...s.notifications].slice(0, MAX_NOTIFICATIONS);
      return {
        notifications: updated,
        unreadCount: updated.filter((n) => !n.isRead).length,
      };
    });
  },

  markRead: (id) => {
    set((s) => {
      const notifications = s.notifications.map((n) =>
        n.id === id ? { ...n, isRead: true } : n
      );
      return {
        notifications,
        unreadCount: notifications.filter((n) => !n.isRead).length,
      };
    });
  },

  removeNotification: (id) => {
    set((s) => {
      const notifications = s.notifications.filter((entry) => entry.id !== id);
      return {
        notifications,
        unreadCount: notifications.filter((n) => !n.isRead).length,
      };
    });
  },

  clearPaneNotifications: (paneId, source) => {
    set((s) => {
      const notifications = s.notifications.filter((entry) => {
        if (entry.paneId !== paneId && entry.panelId !== paneId) {
          return true;
        }
        if (!source) {
          return false;
        }
        return entry.source !== source;
      });
      return {
        notifications,
        unreadCount: notifications.filter((n) => !n.isRead).length,
      };
    });
  },

  markAllRead: () => {
    set((s) => ({
      notifications: s.notifications.map((n) => ({ ...n, isRead: true })),
      unreadCount: 0,
    }));
  },

  markWorkspaceRead: (workspaceId) => {
    set((s) => {
      const notifications = s.notifications.map((n) =>
        n.workspaceId === workspaceId ? { ...n, isRead: true } : n
      );
      return {
        notifications,
        unreadCount: notifications.filter((n) => !n.isRead).length,
      };
    });
  },

  clearAll: () => set({ notifications: [], unreadCount: 0 }),

  getUnreadForWorkspace: (workspaceId) => {
    return get().notifications.filter(
      (n) => n.workspaceId === workspaceId && !n.isRead
    ).length;
  },
}));
