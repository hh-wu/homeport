import { ListTodo } from "lucide-react";

import { usePlanStore, type PlanStep } from "../plan";
import {
  toolFailure,
  toolSuccess,
  type AiTool,
  type ToolExecutionResult,
} from "./types";

const MAX_PLAN_STEPS = 20;
const STATUSES = new Set(["pending", "in_progress", "completed"]);

function updatePlan(args: Record<string, unknown>): ToolExecutionResult {
  const raw = Array.isArray(args.steps) ? args.steps : [];
  const steps: PlanStep[] = [];
  for (const item of raw.slice(0, MAX_PLAN_STEPS)) {
    if (!item || typeof item !== "object") continue;
    const record = item as Record<string, unknown>;
    const content =
      typeof record.content === "string" ? record.content.trim() : "";
    if (!content) continue;
    steps.push({
      content,
      status: STATUSES.has(record.status as string)
        ? (record.status as PlanStep["status"])
        : "pending",
    });
  }
  if (raw.length > 0 && steps.length === 0) {
    return toolFailure(
      'Error: each step needs a non-empty content and a status of "pending", "in_progress", or "completed".',
    );
  }
  usePlanStore.getState().setSteps(steps);
  if (steps.length === 0) {
    return toolSuccess("Plan cleared.");
  }
  const done = steps.filter((step) => step.status === "completed").length;
  return toolSuccess(`Plan updated: ${done}/${steps.length} steps completed.`);
}

export const planTools: AiTool[] = [
  {
    spec: {
      name: "update_plan",
      description:
        "Record or update the step-by-step plan for the current task so the user can follow along. Use it for multi-step work: list the steps up front, then mark each one in_progress or completed as you go. Pass an empty steps array to clear the plan when the task is done.",
      parameters: {
        type: "object",
        properties: {
          steps: {
            type: "array",
            description: "The full plan, replacing the previous one.",
            items: {
              type: "object",
              properties: {
                content: {
                  type: "string",
                  description: "What the step does, in one short phrase.",
                },
                status: {
                  type: "string",
                  enum: ["pending", "in_progress", "completed"],
                },
              },
              required: ["content", "status"],
              additionalProperties: false,
            },
          },
        },
        required: ["steps"],
        additionalProperties: false,
      },
    },
    icon: ListTodo,
    labelKey: "ai.tool.updatePlan",
    execute: async (args) => updatePlan(args),
  },
];
