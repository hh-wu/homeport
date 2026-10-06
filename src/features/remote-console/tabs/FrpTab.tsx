import { useCallback, useState } from "react";
import {
  Activity,
  Copy,
  FileText,
  FolderOpen,
  Monitor,
  Play,
  Plus,
  Radio,
  RefreshCw,
  Server,
  Settings2,
  Square,
} from "lucide-react";

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
import { useI18n } from "@/i18n";
import { ipc } from "@/lib/ipc";
import { errorMessage, toast } from "@/lib/toast";
import { cn } from "@/lib/utils";
import type { RcProxy } from "@/types/models";
import { useOverlayStore } from "@/workbench/overlays";
import {
  useRcLog,
  useRcServerState,
  useRcSetWatchdog,
  useRcStart,
  useRcStatus,
  useRcStop,
} from "../api";
import {
  Card,
  CardTitle,
  ProxyBadge,
  RefreshButton,
  StatTile,
  StatusDot,
} from "../parts";
import {
  copyText,
  formatBytesValue,
  portMapping,
  proxyConnection,
} from "../helpers";

export function FrpTab() {
  const { t } = useI18n();
  const statusQuery = useRcStatus();
  const logQuery = useRcLog();
  const serverQuery = useRcServerState();
  const startFrpc = useRcStart();
  const stopFrpc = useRcStop();
  const setWatchdog = useRcSetWatchdog();
  const openSettings = useOverlayStore((state) => state.openSettings);
  const [confirm, setConfirm] = useState<ConfirmState | null>(null);
  const [tab, setTab] = useState<"setup" | "log">("setup");

  const status = statusQuery.data;
  const running = status?.running ?? false;
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

  const openLog = () =>
    void ipc.remoteConsole
      .openLog()
      .catch((error) =>
        toast.error(t("remote.logFailed"), errorMessage(error)),
      );

  const revealLog = () =>
    void ipc.remoteConsole
      .revealLog()
      .catch((error) =>
        toast.error(t("remote.logFailed"), errorMessage(error)),
      );

  const addAsHost = useCallback(
    async (proxy: RcProxy) => {
      if (!proxy.accessPort) return;
      try {
        const hosts = await ipc.hosts.list();
        const exists = hosts.some(
          (host) =>
            !host.deletedAt &&
            host.address === "127.0.0.1" &&
            host.port === proxy.accessPort,
        );
        if (exists) {
          toast.info(t("remote.hostExists", { label: proxy.name }));
          return;
        }
        const keys = (await ipc.keys.list()).filter((key) => !key.deletedAt);
        const key = keys.length === 1 ? keys[0] : null;
        await ipc.hosts.create({
          label: proxy.name,
          address: "127.0.0.1",
          port: proxy.accessPort,
          authType: key ? "key" : null,
          keyId: key?.id ?? null,
        });
        toast.success(
          t("remote.hostAdded", { label: proxy.name }),
          key ? undefined : t("remote.hostNeedsAuth"),
        );
      } catch (error) {
        toast.error(t("remote.hostAddFailed"), errorMessage(error));
      }
    },
    [t],
  );

  return (
    <>
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
          <ContextMenuItem onSelect={openLog}>
            <FileText /> {t("remote.openLog")}
          </ContextMenuItem>
          <ContextMenuItem onSelect={revealLog}>
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
            <RefreshButton
              loading={serverQuery.isFetching}
              label={t("remote.refresh")}
              onClick={() => void serverQuery.refetch()}
            />
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
                    const mapping = proxy.stale ? null : portMapping(proxy, t);
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
                            <ProxyBadge
                              stale={proxy.stale}
                              online={proxy.online}
                            />
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
                                onSelect={() => copyText(connection.command, t)}
                              >
                                <Copy /> {t("remote.copyCommand")}
                              </ContextMenuItem>
                              {connection.rdp ? (
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
                              ) : (
                                <ContextMenuItem
                                  onSelect={() => void addAsHost(proxy)}
                                >
                                  <Plus /> {t("remote.addAsHost")}
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
              <Button type="button" variant="ghost" size="sm" onClick={openLog}>
                <FileText />
                {t("remote.openLog")}
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={revealLog}
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
                <ContextMenuItem onSelect={openLog}>
                  <FileText /> {t("remote.openLog")}
                </ContextMenuItem>
                <ContextMenuItem onSelect={revealLog}>
                  <FolderOpen /> {t("remote.revealLog")}
                </ContextMenuItem>
              </ContextMenuContent>
            </ContextMenu>
          </div>
        )}
      </Card>

      <ConfirmDialog state={confirm} onClose={() => setConfirm(null)} />
    </>
  );
}
