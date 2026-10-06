import { Brain, SearchCode, Trash2 } from "lucide-react";

import { ipc } from "@/lib/ipc";
import { errorMessage } from "@/lib/toast";
import {
  num,
  optionalStr,
  str,
  toolFailure,
  toolSuccess,
  type AiTool,
  type ToolExecutionResult,
} from "./types";

async function remember(
  args: Record<string, unknown>,
): Promise<ToolExecutionResult> {
  const key = str(args, "key");
  const value = str(args, "value");
  if (!key || !value) return toolFailure("Error: key and value are required.");
  try {
    await ipc.aiMemory.set(key, value);
    return toolSuccess(`Remembered "${key}".`);
  } catch (error) {
    return toolFailure(`Error: ${errorMessage(error)}`);
  }
}

async function recall(
  args: Record<string, unknown>,
): Promise<ToolExecutionResult> {
  try {
    const entries = await ipc.aiMemory.recall(
      optionalStr(args, "query"),
      num(args, "limit"),
    );
    if (entries.length === 0) {
      return toolSuccess(
        "No memories stored yet. Use remember to save durable facts.",
      );
    }
    return toolSuccess(
      JSON.stringify(
        entries.map((entry) => ({ key: entry.key, value: entry.value })),
      ),
    );
  } catch (error) {
    return toolFailure(`Error: ${errorMessage(error)}`);
  }
}

async function forget(
  args: Record<string, unknown>,
): Promise<ToolExecutionResult> {
  const key = str(args, "key");
  if (!key) return toolFailure("Error: key is required.");
  try {
    await ipc.aiMemory.remove(key);
    return toolSuccess(`Forgot "${key}".`);
  } catch (error) {
    return toolFailure(`Error: ${errorMessage(error)}`);
  }
}

export const memoryTools: AiTool[] = [
  {
    spec: {
      name: "remember",
      description:
        "Store a durable fact for future conversations: fleet topology, machine roles, paths, ports, conventions, or user preferences. Never store passwords, keys, or tokens; those belong in the credential vault.",
      parameters: {
        type: "object",
        properties: {
          key: {
            type: "string",
            description:
              'Short label for the fact, e.g. "fleet/vps" or "server-4090/ssh-port".',
          },
          value: {
            type: "string",
            description: "The fact itself, in one or two sentences.",
          },
        },
        required: ["key", "value"],
        additionalProperties: false,
      },
    },
    icon: Brain,
    labelKey: "ai.tool.rememberMemory",
    execute: remember,
  },
  {
    spec: {
      name: "recall",
      description:
        "Look up memories stored with remember. Pass a query to search keys and values, or omit it to list the most recent entries.",
      parameters: {
        type: "object",
        properties: {
          query: {
            type: "string",
            description: "Optional text to search for.",
          },
          limit: {
            type: "number",
            description: "Maximum entries to return (default 20, max 100).",
          },
        },
        additionalProperties: false,
      },
    },
    icon: SearchCode,
    labelKey: "ai.tool.recallMemory",
    execute: recall,
  },
  {
    spec: {
      name: "forget",
      description:
        "Delete a stored memory by its key when it is outdated or wrong.",
      parameters: {
        type: "object",
        properties: {
          key: {
            type: "string",
            description: "The memory key to delete.",
          },
        },
        required: ["key"],
        additionalProperties: false,
      },
    },
    icon: Trash2,
    labelKey: "ai.tool.forgetMemory",
    execute: forget,
  },
];
