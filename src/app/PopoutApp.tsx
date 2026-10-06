import { lazy, Suspense, useEffect, useState } from "react";

import { ErrorBoundary } from "@/components/ui/error-boundary";
import { TITLE_BAR_H } from "@/workbench/layout-sizing";
import type { PopoutPanel } from "@/workbench/popout";
import { useZoomStore } from "@/workbench/zoom";
import { PopoutTitleBar } from "./PopoutTitleBar";

const AssistantPanel = lazy(() =>
  import("@/features/ai/AssistantPanel").then((module) => ({
    default: module.AssistantPanel,
  })),
);

const SftpPanel = lazy(() =>
  import("@/features/sftp/SftpPanel").then((module) => ({
    default: module.SftpPanel,
  })),
);

export function PopoutApp({ panel }: { panel: PopoutPanel }) {
  const [size, setSize] = useState({
    width: window.innerWidth,
    height: window.innerHeight,
  });

  useEffect(() => {
    useZoomStore.getState().init();
    const onResize = () =>
      setSize({ width: window.innerWidth, height: window.innerHeight });
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-surface text-surface-foreground">
      <PopoutTitleBar panel={panel} />
      <div className="flex min-h-0 flex-1">
        <ErrorBoundary>
          <Suspense fallback={null}>
            {panel === "assistant" ? (
              <AssistantPanel width={size.width} />
            ) : (
              <SftpPanel height={size.height - TITLE_BAR_H} />
            )}
          </Suspense>
        </ErrorBoundary>
      </div>
    </div>
  );
}
