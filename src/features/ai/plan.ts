import { create } from "zustand";

export interface PlanStep {
  content: string;
  status: "pending" | "in_progress" | "completed";
}

interface PlanState {
  steps: PlanStep[];
  setSteps: (steps: PlanStep[]) => void;
}

export const usePlanStore = create<PlanState>()((set) => ({
  steps: [],
  setSteps: (steps) => set({ steps }),
}));
