import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { create } from "zustand";

import { detectLocale } from "@/i18n/config";
import { translate } from "@/i18n/translate";
import { ipc } from "@/lib/ipc";
import { errorMessage, toast } from "@/lib/toast";

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

export async function popoutPanel(panel: PopoutPanel): Promise<void> {
  try {
    await ipc.window.popout(panel);
    usePopoutStore.getState().setDetached(panel, true);
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
}

export function usePopoutSync(): void {
  useEffect(() => {
    if (POPOUT_PANEL) return;
    let active = true;
    void ipc.window
      .popouts()
      .then((panels) => {
        if (!active) return;
        for (const panel of panels) {
          if (isPopoutPanel(panel)) {
            usePopoutStore.getState().setDetached(panel, true);
          }
        }
      })
      .catch(() => {});
    const pending = listen<string>("popout://closed", (event) => {
      if (isPopoutPanel(event.payload)) {
        usePopoutStore.getState().setDetached(event.payload, false);
      }
    });
    return () => {
      active = false;
      void pending.then((off) => off());
    };
  }, []);
}
