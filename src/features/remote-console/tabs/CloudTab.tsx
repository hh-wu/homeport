import {
  ArrowDown,
  ArrowUp,
  Cloud,
  Coins,
  Copy,
  Cpu,
  Download,
  Globe,
  MapPin,
  Receipt,
  Server,
  Upload,
  Wallet,
} from "lucide-react";

import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
  Spinner,
} from "@/components/ui";
import { useI18n } from "@/i18n";
import { errorMessage } from "@/lib/toast";
import { useRcCloud } from "../api";
import { Card, CardTitle, RefreshButton, StatTile } from "../parts";
import { copyText, splitGb, splitRate } from "../helpers";

export function CloudTab() {
  const { t } = useI18n();
  const loadCloud = useRcCloud();
  const cloud = loadCloud.data;

  const outRate = splitRate(cloud?.outRate ?? null);
  const inRate = splitRate(cloud?.inRate ?? null);
  const outGb = splitGb(cloud?.outGb ?? null);
  const inGb = splitGb(cloud?.inGb ?? null);
  const instanceStatus = cloud
    ? cloud.instanceStatus === "running"
      ? t("remote.instanceRunning")
      : cloud.instanceStatus === "stopped"
        ? t("remote.instanceStopped")
        : cloud.instanceStatus
    : "—";

  return (
    <Card>
      <CardTitle
        icon={Cloud}
        tone="warning"
        title={t("remote.cloud")}
        actions={
          <RefreshButton
            loading={loadCloud.isPending}
            label={cloud ? t("remote.refresh") : t("remote.load")}
            onClick={() => loadCloud.mutate()}
          />
        }
      />
      {loadCloud.isPending && !cloud ? (
        <div className="flex justify-center py-4">
          <Spinner />
        </div>
      ) : cloud ? (
        <div className="mt-2.5 flex flex-col gap-2.5">
          <div className="grid grid-cols-2 gap-2">
            <StatTile
              icon={Server}
              tone={cloud.instanceStatus === "running" ? "success" : "warning"}
              label={t("remote.instanceStatus")}
              value={instanceStatus}
            />
            <StatTile
              icon={Cpu}
              tone="info"
              label={t("remote.spec")}
              value={cloud.instanceSpec || "—"}
            />
            <StatTile
              icon={ArrowUp}
              tone="warning"
              label={t("remote.outRate")}
              value={outRate.value}
              unit={outRate.unit}
            />
            <StatTile
              icon={ArrowDown}
              tone="info"
              label={t("remote.inRate")}
              value={inRate.value}
              unit={inRate.unit}
            />
            <StatTile
              icon={Upload}
              tone="warning"
              label={t("remote.outTraffic")}
              value={outGb.value}
              unit={outGb.unit}
            />
            <StatTile
              icon={Download}
              tone="info"
              label={t("remote.inTraffic")}
              value={inGb.value}
              unit={inGb.unit}
            />
            <StatTile
              icon={Coins}
              tone="warning"
              label={t("remote.estFee")}
              value={cloud.estFee === null ? "—" : cloud.estFee.toFixed(2)}
              unit={cloud.estFee === null ? undefined : t("remote.feeUnit")}
            />
            <StatTile
              icon={Wallet}
              tone="success"
              label={t("remote.balance")}
              value={cloud.balance ?? "—"}
              unit={cloud.balance ? t("remote.feeUnit") : undefined}
            />
            <StatTile
              icon={Receipt}
              tone="primary"
              label={t("remote.bill")}
              value={cloud.bill ?? "—"}
              unit={cloud.bill ? t("remote.feeUnit") : undefined}
            />
            <StatTile
              icon={MapPin}
              tone="primary"
              label={t("remote.zone")}
              value={cloud.zone || "—"}
            />
          </div>
          <ContextMenu>
            <ContextMenuTrigger asChild>
              <div className="flex items-center justify-between gap-2 rounded-lg border border-border-subtle bg-surface px-2.5 py-2">
                <span className="flex items-center gap-1.5 text-[0.6875rem] text-muted-foreground">
                  <Globe className="size-3" strokeWidth={1.9} />
                  {t("remote.publicIp")}
                </span>
                <span className="font-mono text-[0.6875rem] text-foreground">
                  {cloud.publicIp || "—"}
                </span>
              </div>
            </ContextMenuTrigger>
            <ContextMenuContent>
              <ContextMenuItem
                disabled={!cloud.publicIp}
                onSelect={() => copyText(cloud.publicIp, t)}
              >
                <Copy /> {t("remote.copyIp")}
              </ContextMenuItem>
            </ContextMenuContent>
          </ContextMenu>
          {cloud.errors.length > 0 && (
            <ul className="flex flex-col gap-1">
              {cloud.errors.map((item) => (
                <li key={item} className="text-[0.6875rem] text-destructive">
                  {item}
                </li>
              ))}
            </ul>
          )}
        </div>
      ) : (
        <p className="mt-2.5 text-[0.6875rem] text-muted-foreground">
          {loadCloud.error
            ? errorMessage(loadCloud.error)
            : t("remote.cloudIdle")}
        </p>
      )}
    </Card>
  );
}
