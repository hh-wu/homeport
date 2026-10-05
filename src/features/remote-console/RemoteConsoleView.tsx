import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { RefreshCw, Settings2 } from "lucide-react";

import {
  Button,
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
  SegmentedControl,
} from "@/components/ui";
import { useI18n } from "@/i18n";
import { ipc } from "@/lib/ipc";
import { SideBarView } from "@/workbench/SideBarView";
import { useOverlayStore } from "@/workbench/overlays";
import { remoteConsoleKeys } from "./api";
import { Card, CardTitle } from "./parts";
import { CloudTab } from "./tabs/CloudTab";
import { FrpTab } from "./tabs/FrpTab";
import { RustDeskTab } from "./tabs/RustDeskTab";

type RemoteTab = "frp" | "rustdesk" | "cloud";

export function RemoteConsoleView() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const openSettings = useOverlayStore((state) => state.openSettings);
  const [tab, setTab] = useState<RemoteTab>("frp");

  useEffect(() => {
    const unlisten = ipc.remoteConsole.onStatusChanged(() => {
      void queryClient.invalidateQueries({
        queryKey: remoteConsoleKeys.status,
      });
    });
    return () => void unlisten.then((off) => off());
  }, [queryClient]);

  return (
    <SideBarView
      title={t("remote.viewTitle")}
      actions={
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label={t("remote.refresh")}
          onClick={() =>
            void queryClient.invalidateQueries({
              queryKey: ["remoteConsole"],
            })
          }
        >
          <RefreshCw />
        </Button>
      }
    >
      <div className="flex flex-col gap-2.5 px-3 py-3">
        <SegmentedControl
          value={tab}
          onChange={setTab}
          options={[
            { value: "frp", label: t("remote.tabTunnels") },
            { value: "rustdesk", label: t("remote.tabRustdesk") },
            { value: "cloud", label: t("remote.tabCloud") },
          ]}
        />

        {tab === "frp" && <FrpTab />}
        {tab === "rustdesk" && <RustDeskTab />}
        {tab === "cloud" && <CloudTab />}

        <ContextMenu>
          <ContextMenuTrigger asChild>
            <Card>
              <CardTitle
                icon={Settings2}
                tone="primary"
                title={t("remote.cfgTitle")}
                actions={
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onClick={() => openSettings("remote")}
                  >
                    {t("remote.cfgOpen")}
                  </Button>
                }
              />
              <p className="mt-1.5 text-[0.6875rem] text-muted-foreground">
                {t("remote.cfgMovedHint")}
              </p>
            </Card>
          </ContextMenuTrigger>
          <ContextMenuContent>
            <ContextMenuItem onSelect={() => openSettings("remote")}>
              <Settings2 /> {t("remote.cfgOpen")}
            </ContextMenuItem>
          </ContextMenuContent>
        </ContextMenu>
      </div>
    </SideBarView>
  );
}
