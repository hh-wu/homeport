import { Cloud, KeyRound, RadioTower, SquareTerminal } from "lucide-react";

import { ipc } from "@/lib/ipc";
import { errorMessage } from "@/lib/toast";
import type { RcProxy, RcServerState } from "@/types/models";
import {
  bool,
  str,
  toolFailure,
  toolSuccess,
  type AiTool,
  type ToolExecutionResult,
} from "./types";

function proxyEntry(proxy: RcProxy) {
  const local = proxy.localPort
    ? `${proxy.localIp && proxy.localIp !== "127.0.0.1" ? `${proxy.localIp}:` : ""}${proxy.localPort}`
    : undefined;
  const access = proxy.accessPort ?? proxy.remotePort ?? undefined;
  return {
    name: proxy.name,
    kind: proxy.kind,
    status: proxy.online ? "online" : "offline",
    leftover: proxy.stale || undefined,
    localPort: local,
    accessPort: access,
    accessOnThisMachine: proxy.accessLocal || undefined,
    connections: proxy.curConns,
    lastStartTime: proxy.lastStartTime || undefined,
  };
}

function relayEntry(server: RcServerState) {
  return {
    host: server.relayHost || undefined,
    bindPort: server.bindPort,
    version: server.version || undefined,
    clients: server.clientCounts,
    trafficInBytes: server.totalTrafficIn,
    trafficOutBytes: server.totalTrafficOut,
    errors: server.errors.length ? server.errors : undefined,
  };
}

async function getRemoteStatus(): Promise<ToolExecutionResult> {
  const status = await ipc.remoteConsole.status();
  const frpc = {
    running: status.running,
    pids: status.pids.length ? status.pids : undefined,
    uptime: status.uptime ?? undefined,
    watchdog: status.watchdog,
    lastLogLine: status.lastLog || undefined,
  };

  try {
    const server = await ipc.remoteConsole.serverState();
    return toolSuccess(
      JSON.stringify({
        frpc,
        relay: relayEntry(server),
        tunnels: server.proxies.map(proxyEntry),
      }),
    );
  } catch (error) {
    return toolSuccess(
      JSON.stringify({
        frpc,
        relay: `Error: could not read the relay state: ${errorMessage(error)}`,
      }),
    );
  }
}

async function getRustDeskConfig(): Promise<ToolExecutionResult> {
  const info = await ipc.remoteConsole.rustdesk();
  if (!info.domain && !info.key) {
    return toolFailure(
      "Error: the RustDesk server is not configured. Set it in Settings, Remote access.",
    );
  }
  return toolSuccess(
    JSON.stringify({
      server: info.domain || undefined,
      relay: info.relay || undefined,
      publicKey: info.key || undefined,
      clientConfig: `Server: ${info.domain}\nKey: ${info.key}`,
    }),
  );
}

async function getCloudStatus(): Promise<ToolExecutionResult> {
  const cloud = await ipc.remoteConsole.cloud();
  return toolSuccess(
    JSON.stringify({
      status: cloud.instanceStatus,
      spec: cloud.instanceSpec || undefined,
      zone: cloud.zone || undefined,
      publicIp: cloud.publicIp || undefined,
      outRateBps: cloud.outRate ?? undefined,
      inRateBps: cloud.inRate ?? undefined,
      outGbThisMonth: cloud.outGb ?? undefined,
      inGbThisMonth: cloud.inGb ?? undefined,
      estimatedTrafficFee: cloud.estFee ?? undefined,
      balance: cloud.balance ?? undefined,
      billThisMonth: cloud.bill ?? undefined,
      cycle: cloud.cycle || undefined,
      errors: cloud.errors.length ? cloud.errors : undefined,
    }),
  );
}

async function controlFrpc(
  args: Record<string, unknown>,
): Promise<ToolExecutionResult> {
  const action = str(args, "action");
  if (action === "start") {
    const status = await ipc.remoteConsole.start();
    return toolSuccess(
      `Started frpc${status.pids.length ? ` (pid ${status.pids.join(", ")})` : ""}.`,
    );
  }
  if (action === "stop") {
    await ipc.remoteConsole.stop();
    return toolSuccess(
      "Stopped frpc. Remote access from other machines is down.",
    );
  }
  if (action === "watchdog") {
    const enabled = bool(args, "enabled");
    await ipc.remoteConsole.setWatchdog(enabled);
    return toolSuccess(
      enabled
        ? "Watchdog enabled: frpc restarts within 5 seconds if it stops."
        : "Watchdog disabled.",
    );
  }
  return toolFailure('Error: action must be "start", "stop", or "watchdog".');
}

const FRPC_ACTION_FIELDS = {
  action: {
    type: "string",
    enum: ["start", "stop", "watchdog"],
    description:
      '"start" launches frpc, "stop" terminates it (other machines lose access), "watchdog" toggles auto-restart.',
  },
  enabled: {
    type: "boolean",
    description: 'Required for action "watchdog": true enables auto-restart.',
  },
} as const;

export const remoteTools: AiTool[] = [
  {
    spec: {
      name: "get_remote_status",
      description:
        "Get the remote access state: whether the local frpc client is running (pid, uptime, watchdog), the relay server (frps version, address, connected clients), and every reverse tunnel with its status, ports, and connection counts.",
      parameters: {
        type: "object",
        properties: {},
        additionalProperties: false,
      },
    },
    icon: RadioTower,
    labelKey: "ai.tool.getRemoteStatus",
    execute: getRemoteStatus,
  },
  {
    spec: {
      name: "get_rustdesk_config",
      description:
        "Get the self-hosted RustDesk server address, relay, and public key used by this machine.",
      parameters: {
        type: "object",
        properties: {},
        additionalProperties: false,
      },
    },
    icon: KeyRound,
    labelKey: "ai.tool.getRustdeskConfig",
    execute: getRustDeskConfig,
  },
  {
    spec: {
      name: "get_cloud_status",
      description:
        "Get the Alibaba Cloud ECS instance behind the relay: instance status, spec, zone, public IP, live bandwidth, monthly traffic, estimated traffic fee, account balance, and this month's bill.",
      parameters: {
        type: "object",
        properties: {},
        additionalProperties: false,
      },
    },
    icon: Cloud,
    labelKey: "ai.tool.getCloudStatus",
    execute: getCloudStatus,
  },
  {
    spec: {
      name: "control_frpc",
      description:
        "Start or stop the local frpc tunnel client, or toggle its auto-restart watchdog. Stopping frpc cuts off every remote connection into this machine.",
      parameters: {
        type: "object",
        properties: FRPC_ACTION_FIELDS,
        required: ["action"],
        additionalProperties: false,
      },
    },
    icon: SquareTerminal,
    labelKey: "ai.tool.controlFrpc",
    requiresApproval: true,
    confirmKey: "ai.confirmControlFrpc",
    execute: controlFrpc,
  },
];
