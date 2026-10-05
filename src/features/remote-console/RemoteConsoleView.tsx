import { useCallback, useEffect, useState } from "react";
import { Copy, FileText, Play, RefreshCw, Square } from "lucide-react";

import {
  Button,
  ConfirmDialog,
  SectionHeader,
  Separator,
  Spinner,
  SwitchField,
  type ConfirmState,
} from "@/components/ui";
import { useI18n } from "@/i18n";
import { ipc } from "@/lib/ipc";
import { errorMessage, toast } from "@/lib/toast";
import { cn } from "@/lib/utils";
import type { RcCloud, RcRustDesk, RcStatus } from "@/types/models";
import { SideBarView } from "@/workbench/SideBarView";

const POLL_MS = 3000;

function formatRate(bps: number | null) {
  if (bps === null) return "—";
  if (bps >= 1e6) return `${(bps / 1e6).toFixed(2)} Mbps`;
  if (bps >= 1e3) return `${(bps / 1e3).toFixed(1)} Kbps`;
  return `${bps.toFixed(0)} bps`;
}

function formatGb(gb: number | null) {
  return gb === null ? "—" : `${gb.toFixed(3)} GB`;
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-3 py-0.5">
      <span className="shrink-0 text-xs text-muted-foreground">{label}</span>
      <span className="truncate text-xs font-medium text-foreground">
        {value}
      </span>
    </div>
  );
}

function StatusDot({ ok }: { ok: boolean }) {
  return (
    <span
      aria-hidden
      className={cn(
        "inline-block size-2 shrink-0 rounded-full",
        ok ? "bg-[var(--status-success)]" : "bg-[var(--status-danger)]",
      )}
    />
  );
}

export function RemoteConsoleView() {
  const { t } = useI18n();
  const [status, setStatus] = useState<RcStatus | null>(null);
  const [cloud, setCloud] = useState<RcCloud | null>(null);
  const [rustdesk, setRustdesk] = useState<RcRustDesk | null>(null);
  const [proxies, setProxies] = useState<string | null>(null);
  const [proxiesOk, setProxiesOk] = useState(false);
  const [log, setLog] = useState("");
  const [busy, setBusy] = useState(false);
  const [loadingCloud, setLoadingCloud] = useState(false);
  const [checkingProxies, setCheckingProxies] = useState(false);
  const [confirm, setConfirm] = useState<ConfirmState | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await ipc.remoteConsole.status());
    } catch (error) {
      toast.error(t("remote.statusFailed"), errorMessage(error));
    }
  }, [t]);

  const refreshLog = useCallback(async () => {
    try {
      setLog(await ipc.remoteConsole.logTail(200));
    } catch (error) {
      toast.error(t("remote.logFailed"), errorMessage(error));
    }
  }, [t]);

  const loadCloud = useCallback(async () => {
    setLoadingCloud(true);
    try {
      setCloud(await ipc.remoteConsole.cloud());
    } catch (error) {
      toast.error(t("remote.cloudFailed"), errorMessage(error));
    } finally {
      setLoadingCloud(false);
    }
  }, [t]);

  const checkProxies = useCallback(async () => {
    setCheckingProxies(true);
    try {
      setProxies(await ipc.remoteConsole.checkProxies());
      setProxiesOk(true);
    } catch (error) {
      setProxies(errorMessage(error));
      setProxiesOk(false);
    } finally {
      setCheckingProxies(false);
    }
  }, []);

  useEffect(() => {
    void refreshStatus();
    void refreshLog();
    void ipc.remoteConsole
      .rustdesk()
      .then(setRustdesk)
      .catch((error) => toast.error(t("remote.statusFailed"), errorMessage(error)));
    const timer = window.setInterval(() => void refreshStatus(), POLL_MS);
    const unlisten = ipc.remoteConsole.onStatusChanged(() => void refreshStatus());
    return () => {
      window.clearInterval(timer);
      void unlisten.then((off) => off());
    };
  }, [refreshLog, refreshStatus, t]);

  const startFrpc = async () => {
    setBusy(true);
    try {
      setStatus(await ipc.remoteConsole.start());
      toast.success(t("remote.started"));
      await refreshLog();
    } catch (error) {
      toast.error(t("remote.startFailed"), errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  const stopFrpc = async () => {
    setBusy(true);
    try {
      setStatus(await ipc.remoteConsole.stop());
      toast.success(t("remote.stoppedToast"));
    } catch (error) {
      toast.error(t("remote.stopFailed"), errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  const requestStop = () => {
    setConfirm({
      title: t("remote.stopTitle"),
      description: t("remote.stopDescription"),
      cancelLabel: t("common.cancel"),
      actions: [
        {
          label: t("remote.stopConfirm"),
          variant: "destructive",
          onSelect: () => void stopFrpc(),
        },
      ],
    });
  };

  const running = status?.running ?? false;
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
            void refreshStatus();
            void refreshLog();
            if (cloud) void loadCloud();
          }}
        >
          <RefreshCw />
        </Button>
      }
    >
      <div className="flex flex-col gap-3 px-3 py-3">
        <section className="rounded-md border border-border-subtle bg-surface-raised p-3">
          <SectionHeader
            title={
              <span className="flex items-center gap-2">
                <StatusDot ok={running} />
                {running ? t("remote.running") : t("remote.stopped")}
              </span>
            }
            description={
              status
                ? running
                  ? t("remote.runningDetail", {
                      pid: status.pids.join(", "),
                      uptime: status.uptime ?? "",
                    })
                  : t("remote.stoppedDetail")
                : undefined
            }
          />
          <div className="mt-3 flex gap-2">
            {running ? (
              <Button
                type="button"
                variant="destructive"
                size="sm"
                disabled={busy}
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
                loading={busy}
                onClick={() => void startFrpc()}
              >
                <Play />
                {t("remote.start")}
              </Button>
            )}
          </div>
          <div className="mt-3">
            <SwitchField
              label={t("remote.watchdog")}
              description={t("remote.watchdogHint")}
              checked={status?.watchdog ?? false}
              onCheckedChange={(checked) => {
                void ipc.remoteConsole
                  .setWatchdog(checked)
                  .then(() => void refreshStatus());
              }}
            />
          </div>
        </section>

        <section className="rounded-md border border-border-subtle bg-surface-raised p-3">
          <SectionHeader
            title={t("remote.proxies")}
            description={t("remote.proxiesHint")}
            actions={
              <Button
                type="button"
                variant="ghost"
                size="sm"
                loading={checkingProxies}
                onClick={() => void checkProxies()}
              >
                {t("remote.check")}
              </Button>
            }
          />
          {proxies !== null && (
            <p
              className={cn(
                "mt-2 flex items-start gap-2 text-xs",
                proxiesOk ? "text-foreground" : "text-[var(--status-danger)]",
              )}
            >
              <StatusDot ok={proxiesOk} />
              <span className="break-all">{proxies}</span>
            </p>
          )}
        </section>

        <section className="rounded-md border border-border-subtle bg-surface-raised p-3">
          <SectionHeader
            title={t("remote.cloud")}
            description={t("remote.cloudHint")}
            actions={
              <Button
                type="button"
                variant="ghost"
                size="sm"
                disabled={loadingCloud}
                onClick={() => void loadCloud()}
              >
                {cloud ? t("remote.refresh") : t("remote.load")}
              </Button>
            }
          />
          {loadingCloud && !cloud ? (
            <div className="flex justify-center py-4">
              <Spinner />
            </div>
          ) : cloud ? (
            <div className="mt-2 flex flex-col">
              <Metric label={t("remote.instanceStatus")} value={instanceStatus} />
              <Metric label={t("remote.spec")} value={cloud.instanceSpec || "—"} />
              <Metric label={t("remote.zone")} value={cloud.zone || "—"} />
              <Metric label={t("remote.publicIp")} value={cloud.publicIp || "—"} />
              <Separator className="my-1.5" />
              <Metric label={t("remote.outRate")} value={formatRate(cloud.outRate)} />
              <Metric label={t("remote.inRate")} value={formatRate(cloud.inRate)} />
              <Metric label={t("remote.outTraffic")} value={formatGb(cloud.outGb)} />
              <Metric label={t("remote.inTraffic")} value={formatGb(cloud.inGb)} />
              <Metric
                label={t("remote.estFee")}
                value={cloud.estFee === null ? "—" : t("remote.feeValue", { value: cloud.estFee.toFixed(2) })}
              />
              <Separator className="my-1.5" />
              <Metric label={t("remote.balance")} value={cloud.balance ?? "—"} />
              <Metric label={t("remote.bill")} value={cloud.bill ?? "—"} />
              {cloud.errors.length > 0 && (
                <ul className="mt-2 flex flex-col gap-1">
                  {cloud.errors.map((item) => (
                    <li key={item} className="text-xs text-[var(--status-danger)]">
                      {item}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          ) : (
            <p className="mt-2 text-xs text-muted-foreground">
              {t("remote.cloudIdle")}
            </p>
          )}
        </section>

        <section className="rounded-md border border-border-subtle bg-surface-raised p-3">
          <SectionHeader title={t("remote.rustdesk")} description={t("remote.rustdeskHint")} />
          <div className="mt-2 flex flex-col gap-1">
            <Metric label={t("remote.rustdeskServer")} value={rustdesk?.domain ?? "—"} />
            <Metric label={t("remote.rustdeskRelay")} value={rustdesk?.vps ?? "—"} />
            <p className="mt-1 break-all font-mono text-[0.6875rem] text-muted-foreground">
              {rustdesk?.key ?? ""}
            </p>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="mt-3"
            disabled={!rustdesk}
            onClick={() => {
              if (!rustdesk) return;
              void navigator.clipboard
                .writeText(
                  `${t("remote.rustdeskServer")}: ${rustdesk.domain}\nKey: ${rustdesk.key}\n`,
                )
                .then(() => toast.success(t("remote.copied")));
            }}
          >
            <Copy />
            {t("remote.copy")}
          </Button>
        </section>

        <section className="rounded-md border border-border-subtle bg-surface-raised p-3">
          <SectionHeader
            title={t("remote.log")}
            actions={
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
            }
          />
          <pre className="mt-2 max-h-52 overflow-auto rounded-md bg-surface-sunken p-2 font-mono text-[0.6875rem] leading-relaxed text-muted-foreground">
            {log || t("remote.logEmpty")}
          </pre>
        </section>
      </div>

      <ConfirmDialog state={confirm} onClose={() => setConfirm(null)} />
    </SideBarView>
  );
}
