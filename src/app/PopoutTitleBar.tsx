import { useEffect, useState } from "react";
import { Copy, Minus, Pin, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { INTERACTIVE_FOCUS_CLASS, Tooltip } from "@/components/ui";
import { useI18n } from "@/i18n";
import { errorMessage, toast } from "@/lib/toast";
import { cn } from "@/lib/utils";
import { dockPanel, type PopoutPanel } from "@/workbench/popout";

const appWindow = getCurrentWindow();

export function PopoutTitleBar({ panel }: { panel: PopoutPanel }) {
  const { t } = useI18n();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let active = true;
    const refresh = () =>
      void appWindow
        .isMaximized()
        .then((value) => {
          if (active) setMaximized(value);
        })
        .catch(() => {});
    refresh();
    const pending = appWindow.onResized(refresh);
    return () => {
      active = false;
      void pending.then((off) => off());
    };
  }, []);

  const run = (action: () => Promise<unknown>) => {
    void action().catch((error) =>
      toast.error(t("windowControls.actionError"), errorMessage(error)),
    );
  };

  const title =
    panel === "assistant" ? t("ai.viewTitle") : t("sftp.panelTitle");

  const buttonClass = cn(
    "flex h-full w-10 items-center justify-center text-surface-foreground/70 transition-colors hover:bg-black/[0.06] dark:hover:bg-white/10",
    INTERACTIVE_FOCUS_CLASS,
  );

  return (
    <header
      data-tauri-drag-region
      className="flex h-[var(--titlebar-height)] shrink-0 items-center justify-between border-b border-border-subtle bg-surface"
    >
      <div
        data-tauri-drag-region
        className="flex h-full min-w-0 flex-1 items-center pl-3"
      >
        <span
          data-tauri-drag-region
          className="truncate text-xs font-medium text-surface-foreground"
        >
          {title}
        </span>
      </div>
      <div className="flex h-full items-center">
        <Tooltip content={t("popout.dockBack")}>
          <button
            type="button"
            aria-label={t("popout.dockBack")}
            className={buttonClass}
            onClick={() => void dockPanel(panel)}
          >
            <Pin className="size-4" />
          </button>
        </Tooltip>
        <button
          type="button"
          aria-label={t("windowControls.minimize")}
          className={buttonClass}
          onClick={() => run(() => appWindow.minimize())}
        >
          <Minus className="size-4" />
        </button>
        <button
          type="button"
          aria-label={
            maximized
              ? t("windowControls.restore")
              : t("windowControls.maximize")
          }
          className={buttonClass}
          onClick={() => run(() => appWindow.toggleMaximize())}
        >
          {maximized ? (
            <Copy className="size-3.5 -scale-x-100" />
          ) : (
            <Square className="size-3.5" />
          )}
        </button>
        <button
          type="button"
          aria-label={t("windowControls.close")}
          className={cn(
            buttonClass,
            "hover:bg-destructive hover:text-destructive-foreground",
          )}
          onClick={() => void dockPanel(panel)}
        >
          <X className="size-4" />
        </button>
      </div>
    </header>
  );
}
