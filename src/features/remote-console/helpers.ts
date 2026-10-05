import { errorMessage, toast } from "@/lib/toast";
import type { TFunction } from "@/i18n";
import type { RcProxy } from "@/types/models";

export type Tone = "primary" | "info" | "success" | "warning" | "destructive";

export const TONE_CLASS: Record<Tone, string> = {
  primary: "bg-primary/15 text-primary",
  info: "bg-info/15 text-info",
  success: "bg-success/15 text-success",
  warning: "bg-warning/15 text-warning",
  destructive: "bg-destructive/15 text-destructive",
};

export function copyText(value: string, t: TFunction): void {
  void navigator.clipboard
    .writeText(value)
    .then(() => toast.success(t("remote.copied")))
    .catch((error) => toast.error(t("remote.copyFailed"), errorMessage(error)));
}

export function splitRate(bps: number | null): {
  value: string;
  unit?: string;
} {
  if (bps === null) return { value: "—" };
  if (bps >= 1e6) return { value: (bps / 1e6).toFixed(2), unit: "Mbps" };
  if (bps >= 1e3) return { value: (bps / 1e3).toFixed(1), unit: "Kbps" };
  return { value: bps.toFixed(0), unit: "bps" };
}

export function splitGb(gb: number | null): { value: string; unit?: string } {
  return gb === null ? { value: "—" } : { value: gb.toFixed(3), unit: "GB" };
}

export function formatBytesValue(bytes: number) {
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

export function portMapping(proxy: RcProxy, t: TFunction): string | null {
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

export function proxyConnection(
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
