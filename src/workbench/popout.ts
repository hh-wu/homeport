import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { create } from "zustand";

import { detectLocale } from "@/i18n/config";
import { translate } from "@/i18n/translate";
import { ipc } from "@/lib/ipc";
import { errorMessage, toast } from "@/lib/toast";
import { useLayoutStore } from "./layout";

export type PopoutPanel = "assistant" | "files";

export const POPOUT_PANEL: PopoutPanel | null = (() => {
  const label = getCurrentWindow().label;
  if (label === "popout-assistant") return "assistant";
  if (label === "popout-files") return "files";
  return null;
})();

function isPopoutPanel(value: string): value is PopoutPanel {
  return value === "assistant" || value === "files";
}

interface PopoutState {
  detached: Record<PopoutPanel, boolean>;
  setDetached: (panel: PopoutPanel, detached: boolean) => void;
}

export const usePopoutStore = create<PopoutState>()((set) => ({
  detached: { assistant: false, files: false },
  setDetached: (panel, detached) =>
    set((state) => ({ detached: { ...state.detached, [panel]: detached } })),
}));

function hidePanelArea(panel: PopoutPanel): void {
  const layout = useLayoutStore.getState();
  if (panel === "files") layout.setPanelVisible(false);
  else if (layout.auxVisible) layout.toggleAux();
}

function revealPanelArea(panel: PopoutPanel): void {
  const layout = useLayoutStore.getState();
  if (panel === "files") layout.setPanelVisible(true);
  else if (!layout.auxVisible) layout.toggleAux();
}

export function toggleFilesPanel(): void {
  if (usePopoutStore.getState().detached.files) {
    void dockPanel("files");
    return;
  }
  useLayoutStore.getState().togglePanel();
}

export function toggleAssistantPanel(): void {
  if (usePopoutStore.getState().detached.assistant) {
    void dockPanel("assistant");
    return;
  }
  useLayoutStore.getState().toggleAux();
}

export async function popoutPanel(panel: PopoutPanel): Promise<void> {
  try {
    await ipc.window.popout(panel);
    usePopoutStore.getState().setDetached(panel, true);
    hidePanelArea(panel);
  } catch (error) {
    toast.error(
      translate(detectLocale(), "popout.openFailed"),
      errorMessage(error),
    );
  }
}

export async function dockPanel(panel: PopoutPanel): Promise<void> {
  try {
    await ipc.window.dock(panel);
  } catch {
    void 0;
  }
  usePopoutStore.getState().setDetached(panel, false);
  revealPanelArea(panel);
}

async function syncDetachedFromBackend(): Promise<void> {
  try {
    const panels = await ipc.window.popouts();
    const store = usePopoutStore.getState();
    store.setDetached("assistant", panels.includes("assistant"));
    store.setDetached("files", panels.includes("files"));
  } catch {
    void 0;
  }
}

export function usePopoutSync(): void {
  useEffect(() => {
    if (POPOUT_PANEL) return;
    void syncDetachedFromBackend();
    const pending = listen<string>("popout://closed", (event) => {
      if (isPopoutPanel(event.payload)) {
        usePopoutStore.getState().setDetached(event.payload, false);
        revealPanelArea(event.payload);
      }
    });
    const onFocus = () => void syncDetachedFromBackend();
    window.addEventListener("focus", onFocus);
    return () => {
      window.removeEventListener("focus", onFocus);
      void pending.then((off) => off());
    };
  }, []);
}
