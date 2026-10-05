import { useCallback, useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  Activity,
  ArrowDown,
  ArrowUp,
  Cloud,
  Coins,
  Copy,
  Cpu,
  Download,
  FileText,
  FolderOpen,
  Globe,
  MapPin,
  Monitor,
  Play,
  Radio,
  Receipt,
  RefreshCw,
  Server,
  Settings2,
  Square,
  Upload,
  Wallet,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import {
  Badge,
  Button,
  ConfirmDialog,
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
  SegmentedControl,
  Spinner,
  SwitchField,
  type ConfirmState,
} from "@/components/ui";
import { useI18n, type TFunction } from "@/i18n";
import { ipc } from "@/lib/ipc";
import { errorMessage, toast } from "@/lib/toast";
import { cn } from "@/lib/utils";
import type { RcProxy } from "@/types/models";
import { SideBarView } from "@/workbench/SideBarView";
import {
  remoteConsoleKeys,
  useRcCloud,
  useRcLog,
  useRcRustDesk,
  useRcServerState,
  useRcSetWatchdog,
  useRcStart,
  useRcStatus,
  useRcStop,
} from "./api";
import { useOverlayStore } from "@/workbench/overlays";

type Tone = "primary" | "info" | "success" | "warning" | "destructive";

const TONE_CLASS: Record<Tone, string> = {
  primary: "bg-primary/15 text-primary",
  info: "bg-info/15 text-info",
  success: "bg-success/15 text-success",
  warning: "bg-warning/15 text-warning",
  destructive: "bg-destructive/15 text-destructive",
};

function Card({
  children,
  className,
  ...props
}: React.ComponentProps<"section">) {
  return (
    <section
      className={cn(
        "flex flex-col rounded-lg border border-border-subtle bg-surface-raised p-3",
        className,
      )}
      {...props}
    >
      {children}
    </section>
  );
}

function copyText(value: string, t: TFunction): void {
  void navigator.clipboard
    .writeText(value)
    .then(() => toast.success(t("remote.copied")))
    .catch((error) => toast.error(t("remote.copyFailed"), errorMessage(error)));
}

function CardTitle({
  icon: Icon,
  tone,
  title,
  actions,
}: {
  icon: LucideIcon;
  tone: Tone;
  title: string;
  actions?: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-2">
      <div className="flex min-w-0 items-center gap-2">
        <span
          className={cn(
            "flex size-6 shrink-0 items-center justify-center rounded-md",
            TONE_CLASS[tone],
          )}
        >
          <Icon className="size-3.5" strokeWidth={1.9} />
        </span>
        <h3 className="truncate text-[0.8125rem] font-semibold tracking-[-0.01em] text-foreground">
          {title}
        </h3>
      </div>
      {actions && (
        <div className="flex shrink-0 items-center gap-1">{actions}</div>
      )}
    </div>
  );
}

function StatTile({
  icon: Icon,
  tone,
  label,
  value,
  unit,
  copyValue,
}: {
  icon: LucideIcon;
  tone: Tone;
  label: string;
  value: string;
  unit?: string;
  copyValue?: string;
}) {
  const { t } = useI18n();
  const tile = (
    <div className="flex min-w-0 flex-col gap-1.5 rounded-lg border border-border-subtle bg-surface p-2.5">
      <span
        className={cn(
          "flex size-6 shrink-0 items-center justify-center rounded-md",
          TONE_CLASS[tone],
        )}
      >
        <Icon className="size-3.5" strokeWidth={1.9} />
      </span>
      <span className="truncate text-[0.6875rem] text-muted-foreground">
        {label}
      </span>
      <span className="flex items-baseline gap-0.5">
        <span className="truncate text-sm font-semibold text-foreground">
          {value}
        </span>
        {unit && (
          <span className="text-[0.625rem] text-muted-foreground">{unit}</span>
        )}
      </span>
    </div>
  );

  if (!copyValue && value === "—") return tile;

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{tile}</ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem
          onSelect={() =>
            copyText(copyValue ?? `${value}${unit ? ` ${unit}` : ""}`, t)
          }
        >
          <Copy /> {t("remote.copyValue")}
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}

function StatusDot({ ok }: { ok: boolean }) {
  return (
    <span
      aria-hidden
      className={cn(
        "inline-block size-1.5 shrink-0 rounded-full",
        ok ? "bg-success" : "bg-destructive",
      )}
    />
  );
}

function splitRate(bps: number | null): { value: string; unit?: string } {
  if (bps === null) return { value: "—" };
  if (bps >= 1e6) return { value: (bps / 1e6).toFixed(2), unit: "Mbps" };
  if (bps >= 1e3) return { value: (bps / 1e3).toFixed(1), unit: "Kbps" };
  return { value: bps.toFixed(0), unit: "bps" };
}

function splitGb(gb: number | null): { value: string; unit?: string } {
  return gb === null ? { value: "—" } : { value: gb.toFixed(3), unit: "GB" };
}

function formatBytesValue(bytes: number) {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${unit === 0 ? value : value.toFixed(1)} ${units[unit]}`;
}

const LOOPBACK = ["", "127.0.0.1", "localhost", "::1"];

function portMapping(proxy: RcProxy, t: TFunction): string | null {
  const local = proxy.localPort
    ? `${LOOPBACK.includes(proxy.localIp) ? "" : `${proxy.localIp}:`}${proxy.localPort}`
    : null;
  const target = proxy.remotePort
    ? `${t("remote.mapRelay")} ${proxy.remotePort}`
    : proxy.accessPort
      ? `${t("remote.mapAccess")} ${proxy.accessPort}`
      : null;

  if (local && target) return `${t("remote.mapLocal")} ${local} → ${target}`;
  if (target) return target;
  if (local) return `${t("remote.mapLocal")} ${local}`;
  return null;
}

function proxyConnection(
  proxy: RcProxy,
  t: TFunction,
): { rdp: boolean; command: string } | null {
  if (proxy.stale || !proxy.accessLocal || !proxy.accessPort) return null;
  const port = proxy.accessPort;
  if (proxy.name.endsWith("-rdp")) {
    return { rdp: true, command: `mstsc /v:127.0.0.1:${port}` };
  }
  return { rdp: false, command: t("remote.sshCommand", { port }) };
}

export function RemoteConsoleView() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const statusQuery = useRcStatus();
  const logQuery = useRcLog();
  const rustdeskQuery = useRcRustDesk();
  const serverQuery = useRcServerState();
  const startFrpc = useRcStart();
  const stopFrpc = useRcStop();
  const setWatchdog = useRcSetWatchdog();
  const loadCloud = useRcCloud();
  const openSettings = useOverlayStore((state) => state.openSettings);
  const [confirm, setConfirm] = useState<ConfirmState | null>(null);
  const [tab, setTab] = useState<"setup" | "log">("setup");

  useEffect(() => {
    const unlisten = ipc.remoteConsole.onStatusChanged(() => {
      void queryClient.invalidateQueries({
        queryKey: remoteConsoleKeys.status,
      });
    });
    return () => void unlisten.then((off) => off());
  }, [queryClient]);

  const status = statusQuery.data;
  const running = status?.running ?? false;
  const cloud = loadCloud.data;
  const rustdesk = rustdeskQuery.data;
  const server = serverQuery.data;

  const runStart = useCallback(async () => {
    try {
      await startFrpc.mutateAsync();
      toast.success(t("remote.started"));
    } catch (error) {
      toast.error(t("remote.startFailed"), errorMessage(error));
    }
  }, [startFrpc, t]);

  const requestStop = () => {
    setConfirm({
      title: t("remote.stopTitle"),
      description: t("remote.stopDescription"),
      cancelLabel: t("common.cancel"),
      actions: [
        {
          label: t("remote.stopConfirm"),
          variant: "destructive",
          onSelect: async () => {
            try {
              await stopFrpc.mutateAsync();
              toast.success(t("remote.stoppedToast"));
            } catch (error) {
              toast.error(t("remote.stopFailed"), errorMessage(error));
            }
          },
        },
      ],
    });
  };

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
    <SideBarView
      title={t("remote.viewTitle")}
      actions={
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label={t("remote.refresh")}
          onClick={() => {
            void statusQuery.refetch();
            void logQuery.refetch();
            void serverQuery.refetch();
            if (cloud) loadCloud.mutate();
          }}
        >
          <RefreshCw />
        </Button>
      }
    >
      <div className="flex flex-col gap-2.5 px-3 py-3">
        <ContextMenu>
          <ContextMenuTrigger asChild>
            <Card>
              <CardTitle
                icon={Activity}
                tone={running ? "success" : "destructive"}
                title={running ? t("remote.running") : t("remote.stopped")}
                actions={
                  <Badge variant={running ? "success" : "destructive"}>
                    {running ? t("remote.online") : t("remote.offline")}
                  </Badge>
                }
              />
              <p className="mt-2 text-[0.6875rem] text-muted-foreground">
                {status
                  ? running
                    ? t("remote.runningDetail", {
                        pid: status.pids.join(", "),
                        uptime: status.uptime ?? "",
                      })
                    : t("remote.stoppedDetail")
                  : "…"}
              </p>
              <div className="mt-2.5">
                {running ? (
                  <Button
                    type="button"
                    variant="destructive"
                    size="sm"
                    className="w-full"
                    disabled={stopFrpc.isPending}
                    onClick={requestStop}
                  >
                    <Square />
                    {t("remote.stop")}
                  </Button>
                ) : (
                  <Button
                    type="button"
                    variant="primary"
                    size="sm"
                    className="w-full"
                    loading={startFrpc.isPending}
                    onClick={() => void runStart()}
                  >
                    <Play />
                    {t("remote.start")}
                  </Button>
                )}
              </div>
              <div className="mt-2.5">
                <SwitchField
                  label={t("remote.watchdog")}
                  description={t("remote.watchdogHint")}
                  checked={status?.watchdog ?? false}
                  disabled={setWatchdog.isPending}
                  onCheckedChange={(checked) => setWatchdog.mutate(checked)}
                />
              </div>
            </Card>
          </ContextMenuTrigger>
          <ContextMenuContent>
            {running ? (
              <ContextMenuItem destructive onSelect={requestStop}>
                <Square /> {t("remote.stop")}
              </ContextMenuItem>
            ) : (
              <ContextMenuItem onSelect={() => void runStart()}>
                <Play /> {t("remote.start")}
              </ContextMenuItem>
            )}
            <ContextMenuItem
              onSelect={() => setWatchdog.mutate(!(status?.watchdog ?? false))}
            >
              <Activity /> {t("remote.watchdog")}
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem
              disabled={!status?.pids.length}
              onSelect={() => copyText(status?.pids.join(", ") ?? "", t)}
            >
              <Copy /> {t("remote.copyPid")}
            </ContextMenuItem>
            <ContextMenuItem
              onSelect={() =>
                void ipc.remoteConsole
                  .openLog()
                  .catch((error) =>
                    toast.error(t("remote.logFailed"), errorMessage(error)),
                  )
              }
            >
              <FileText /> {t("remote.openLog")}
            </ContextMenuItem>
            <ContextMenuItem
              onSelect={() =>
                void ipc.remoteConsole
                  .revealLog()
                  .catch((error) =>
                    toast.error(t("remote.logFailed"), errorMessage(error)),
                  )
              }
            >
              <FolderOpen /> {t("remote.revealLog")}
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem onSelect={() => openSettings("remote")}>
              <Settings2 /> {t("remote.cfgOpen")}
            </ContextMenuItem>
          </ContextMenuContent>
        </ContextMenu>

        <Card>
          <CardTitle
            icon={Server}
            tone="info"
            title={t("remote.serverTitle")}
            actions={
              <Button
                type="button"
                variant="ghost"
                size="sm"
                loading={serverQuery.isFetching}
                onClick={() => void serverQuery.refetch()}
              >
                {t("remote.refresh")}
              </Button>
            }
          />
          <SegmentedControl
            className="mt-2.5"
            value={tab}
            onChange={setTab}
            options={[
              { value: "setup", label: t("remote.tabSetup") },
              { value: "log", label: t("remote.tabLog") },
            ]}
          />
          {tab === "setup" ? (
            server ? (
              <div className="mt-2.5 flex flex-col gap-2.5">
                <div className="grid grid-cols-2 gap-2">
                  <StatTile
                    icon={Server}
                    tone="info"
                    label={t("remote.serverVersion")}
                    value={server.version || "—"}
                  />
                  <StatTile
                    icon={Radio}
                    tone="success"
                    label={t("remote.serverClients")}
                    value={String(server.clientCounts)}
                  />
                </div>
                <ContextMenu>
                  <ContextMenuTrigger asChild>
                    <div className="flex items-center justify-between gap-2 rounded-lg border border-border-subtle bg-surface px-2.5 py-2">
                      <span className="text-[0.6875rem] text-muted-foreground">
                        {t("remote.mapRelay")}
                      </span>
                      <span className="truncate font-mono text-[0.6875rem] text-foreground">
                        {server.relayHost}:{server.bindPort}
                      </span>
                    </div>
                  </ContextMenuTrigger>
                  <ContextMenuContent>
                    <ContextMenuItem
                      onSelect={() =>
                        copyText(`${server.relayHost}:${server.bindPort}`, t)
                      }
                    >
                      <Copy /> {t("remote.copyRelay")}
                    </ContextMenuItem>
                  </ContextMenuContent>
                </ContextMenu>
                <ContextMenu>
                  <ContextMenuTrigger asChild>
                    <div className="flex items-center justify-between gap-2 rounded-lg border border-border-subtle bg-surface px-2.5 py-2">
                      <span className="text-[0.6875rem] text-muted-foreground">
                        {t("remote.serverTraffic")}
                      </span>
                      <span className="font-mono text-[0.6875rem] text-foreground">
                        {formatBytesValue(server.totalTrafficIn)} /{" "}
                        {formatBytesValue(server.totalTrafficOut)}
                      </span>
                    </div>
                  </ContextMenuTrigger>
                  <ContextMenuContent>
                    <ContextMenuItem
                      onSelect={() =>
                        copyText(
                          `${formatBytesValue(server.totalTrafficIn)} / ${formatBytesValue(server.totalTrafficOut)}`,
                          t,
                        )
                      }
                    >
                      <Copy /> {t("remote.copyTraffic")}
                    </ContextMenuItem>
                  </ContextMenuContent>
                </ContextMenu>
                <div className="flex flex-col gap-1">
                  {server.proxies.length === 0 ? (
                    <p className="text-[0.6875rem] text-muted-foreground">
                      {t("remote.proxyNone")}
                    </p>
                  ) : (
                    server.proxies.map((proxy) => {
                      const mapping = proxy.stale
                        ? null
                        : portMapping(proxy, t);
                      const connection = proxyConnection(proxy, t);
                      const row = (
                        <div
                          className={cn(
                            "flex flex-col gap-0.5 rounded-md px-1.5 py-1 hover:bg-list-hover",
                            proxy.stale && "opacity-55",
                          )}
                        >
                          <div className="flex items-center justify-between gap-2">
                            <span className="flex min-w-0 items-center gap-2">
                              <StatusDot ok={proxy.online} />
                              <span className="truncate text-xs font-medium text-foreground">
                                {proxy.name}
                              </span>
                            </span>
                            <span className="flex shrink-0 items-center gap-1.5">
                              <span className="text-[0.625rem] text-muted-foreground">
                                {proxy.kind}
                              </span>
                              <Badge
                                variant={
                                  proxy.stale
                                    ? "outline"
                                    : proxy.online
                                      ? "success"
                                      : "destructive"
                                }
                              >
                                {proxy.stale
                                  ? t("remote.stale")
                                  : proxy.online
                                    ? t("remote.online")
                                    : t("remote.offline")}
                              </Badge>
                            </span>
                          </div>
                          {mapping && (
                            <span className="pl-3.5 font-mono text-[0.625rem] text-muted-foreground">
                              {mapping}
                            </span>
                          )}
                        </div>
                      );
                      return (
                        <ContextMenu key={`${proxy.kind}-${proxy.name}`}>
                          <ContextMenuTrigger asChild>{row}</ContextMenuTrigger>
                          <ContextMenuContent>
                            <ContextMenuItem
                              onSelect={() => copyText(proxy.name, t)}
                            >
                              <Copy /> {t("remote.copyProxyName")}
                            </ContextMenuItem>
                            {proxy.accessPort !== null && (
                              <ContextMenuItem
                                onSelect={() =>
                                  copyText(String(proxy.accessPort), t)
                                }
                              >
                                <Copy /> {t("remote.copyAccessPort")}
                              </ContextMenuItem>
                            )}
                            {mapping && (
                              <ContextMenuItem
                                onSelect={() => copyText(mapping, t)}
                              >
                                <Copy /> {t("remote.copyMapping")}
                              </ContextMenuItem>
                            )}
                            {connection && (
                              <>
                                <ContextMenuSeparator />
                                <ContextMenuItem
                                  onSelect={() =>
                                    copyText(connection.command, t)
                                  }
                                >
                                  <Copy /> {t("remote.copyCommand")}
                                </ContextMenuItem>
                                {connection.rdp && (
                                  <ContextMenuItem
                                    onSelect={() =>
                                      void ipc.remoteConsole
                                        .openRdp(proxy.accessPort ?? 0)
                                        .catch((error) =>
                                          toast.error(
                                            t("remote.rdpFailed"),
                                            errorMessage(error),
                                          ),
                                        )
                                    }
                                  >
                                    <Monitor /> {t("remote.connectRdp")}
                                  </ContextMenuItem>
                                )}
                              </>
                            )}
                            <ContextMenuSeparator />
                            <ContextMenuItem
                              onSelect={() => void serverQuery.refetch()}
                            >
                              <RefreshCw /> {t("remote.refresh")}
                            </ContextMenuItem>
                          </ContextMenuContent>
                        </ContextMenu>
                      );
                    })
                  )}
                </div>
                {server.errors.length > 0 && (
                  <ul className="flex flex-col gap-1">
                    {server.errors.map((item) => (
                      <li key={item} className="text-[0.6875rem] text-danger">
                        {item}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            ) : serverQuery.error ? (
              <p className="mt-2.5 text-[0.6875rem] text-danger">
                {errorMessage(serverQuery.error)}
              </p>
            ) : (
              <div className="flex justify-center py-4">
                <Spinner />
              </div>
            )
          ) : (
            <div className="mt-2.5">
              <div className="mt-2 flex flex-wrap gap-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() =>
                    void ipc.remoteConsole
                      .openLog()
                      .catch((error) =>
                        toast.error(t("remote.logFailed"), errorMessage(error)),
                      )
                  }
                >
                  <FileText />
                  {t("remote.openLog")}
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() =>
                    void ipc.remoteConsole
                      .revealLog()
                      .catch((error) =>
                        toast.error(t("remote.logFailed"), errorMessage(error)),
                      )
                  }
                >
                  <FolderOpen />
                  {t("remote.revealLog")}
                </Button>
              </div>
              <ContextMenu>
                <ContextMenuTrigger asChild>
                  <pre className="mt-2 max-h-72 overflow-auto rounded-lg bg-surface-sunken p-2 font-mono text-[0.6875rem] leading-relaxed text-muted-foreground">
                    {logQuery.data || t("remote.logEmpty")}
                  </pre>
                </ContextMenuTrigger>
                <ContextMenuContent>
                  <ContextMenuItem
                    disabled={!logQuery.data}
                    onSelect={() => copyText(logQuery.data ?? "", t)}
                  >
                    <Copy /> {t("remote.copyLog")}
                  </ContextMenuItem>
                  <ContextMenuItem onSelect={() => void logQuery.refetch()}>
                    <RefreshCw /> {t("remote.refreshLog")}
                  </ContextMenuItem>
                  <ContextMenuSeparator />
                  <ContextMenuItem
                    onSelect={() =>
                      void ipc.remoteConsole
                        .openLog()
                        .catch((error) =>
                          toast.error(
                            t("remote.logFailed"),
                            errorMessage(error),
                          ),
                        )
                    }
                  >
                    <FileText /> {t("remote.openLog")}
                  </ContextMenuItem>
                  <ContextMenuItem
                    onSelect={() =>
                      void ipc.remoteConsole
                        .revealLog()
                        .catch((error) =>
                          toast.error(
                            t("remote.logFailed"),
                            errorMessage(error),
                          ),
                        )
                    }
                  >
                    <FolderOpen /> {t("remote.revealLog")}
                  </ContextMenuItem>
                </ContextMenuContent>
              </ContextMenu>{" "}
            </div>
          )}
        </Card>

        <Card>
          <CardTitle
            icon={Cloud}
            tone="warning"
            title={t("remote.cloud")}
            actions={
              <Button
                type="button"
                variant="ghost"
                size="sm"
                loading={loadCloud.isPending}
                onClick={() => loadCloud.mutate()}
              >
                {cloud ? t("remote.refresh") : t("remote.load")}
              </Button>
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
                  tone={
                    cloud.instanceStatus === "running" ? "success" : "warning"
                  }
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
                    <li
                      key={item}
                      className="text-[0.6875rem] text-destructive"
                    >
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

        <ContextMenu>
          <ContextMenuTrigger asChild>
            <Card>
              <CardTitle
                icon={Radio}
                tone="success"
                title={t("remote.rustdesk")}
                actions={
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    disabled={!rustdesk}
                    onClick={() => {
                      if (!rustdesk) return;
                      copyText(
                        `${t("remote.rustdeskServer")}: ${rustdesk.domain}\nKey: ${rustdesk.key}\n`,
                        t,
                      );
                    }}
                  >
                    <Copy />
                    {t("remote.copy")}
                  </Button>
                }
              />
              <div className="mt-2.5 flex flex-col gap-1.5">
                <div className="flex items-center justify-between gap-2">
                  <span className="text-[0.6875rem] text-muted-foreground">
                    {t("remote.rustdeskServer")}
                  </span>
                  <span className="truncate font-mono text-[0.6875rem] text-foreground">
                    {rustdesk?.domain || "—"}
                  </span>
                </div>
                <div className="flex items-center justify-between gap-2">
                  <span className="text-[0.6875rem] text-muted-foreground">
                    {t("remote.rustdeskRelay")}
                  </span>
                  <span className="truncate font-mono text-[0.6875rem] text-foreground">
                    {rustdesk?.relay || "—"}
                  </span>
                </div>
                <p className="rounded-lg bg-surface-sunken px-2 py-1.5 font-mono text-[0.625rem] leading-relaxed break-all text-muted-foreground">
                  {rustdesk?.key || "—"}
                </p>
              </div>
            </Card>
          </ContextMenuTrigger>
          <ContextMenuContent>
            <ContextMenuItem
              disabled={!rustdesk?.domain}
              onSelect={() => copyText(rustdesk?.domain ?? "", t)}
            >
              <Copy /> {t("remote.copyServerAddress")}
            </ContextMenuItem>
            <ContextMenuItem
              disabled={!rustdesk?.key}
              onSelect={() => copyText(rustdesk?.key ?? "", t)}
            >
              <Copy /> {t("remote.copyKey")}
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuItem
              disabled={!rustdesk}
              onSelect={() => {
                if (!rustdesk) return;
                copyText(
                  `${t("remote.rustdeskServer")}: ${rustdesk.domain}\nKey: ${rustdesk.key}\n`,
                  t,
                );
              }}
            >
              <Copy /> {t("remote.copyBlock")}
            </ContextMenuItem>
          </ContextMenuContent>
        </ContextMenu>

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

      <ConfirmDialog state={confirm} onClose={() => setConfirm(null)} />
    </SideBarView>
  );
}
