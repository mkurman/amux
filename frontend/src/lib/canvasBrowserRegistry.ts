import type { BrowserDomSnapshot } from "./browserRegistry";

export type CanvasBrowserController = {
  getUrl: () => string;
  getTitle: () => string;
  getDomSnapshot: () => Promise<BrowserDomSnapshot>;
};

const controllers = new Map<string, CanvasBrowserController>();

export function registerCanvasBrowserController(
  paneId: string,
  controller: CanvasBrowserController,
): () => void {
  controllers.set(paneId, controller);
  return () => {
    if (controllers.get(paneId) === controller) {
      controllers.delete(paneId);
    }
  };
}

export function getCanvasBrowserController(
  paneId: string,
): CanvasBrowserController | undefined {
  return controllers.get(paneId);
}

