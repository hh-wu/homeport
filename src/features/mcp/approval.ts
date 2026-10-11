import { create } from "zustand";

import { ipc } from "@/lib/ipc";
import type { McpApprovalEvent } from "@/types/models";

interface McpApprovalState {
  queue: McpApprovalEvent[];
  push: (event: McpApprovalEvent) => void;
  dismiss: (requestId: string) => void;
  respond: (requestId: string, approved: boolean) => void;
}

export const useMcpApprovalStore = create<McpApprovalState>((set, get) => ({
  queue: [],

  push: (event) =>
    set((state) =>
      state.queue.some((item) => item.requestId === event.requestId)
        ? state
        : { queue: [...state.queue, event] },
    ),

  dismiss: (requestId) =>
    set((state) => ({
      queue: state.queue.filter((event) => event.requestId !== requestId),
    })),

  respond: (requestId, approved) => {
    const request = get().queue.find((event) => event.requestId === requestId);
    if (!request) return;
    set((state) => ({
      queue: state.queue.filter((event) => event.requestId !== requestId),
    }));
    void ipc.mcp.resolveApproval(requestId, approved).catch(() => {
      set((state) =>
        state.queue.some((event) => event.requestId === requestId)
          ? state
          : { queue: [request, ...state.queue] },
      );
    });
  },
}));

export async function listenMcpApprovals(): Promise<() => void> {
  const unlistenRequest = await ipc.mcp.onApprovalRequest((event) =>
    useMcpApprovalStore.getState().push(event),
  );
  let unlistenClosed: (() => void) | undefined;
  try {
    unlistenClosed = await ipc.mcp.onApprovalClosed((event) =>
      useMcpApprovalStore.getState().dismiss(event.requestId),
    );
  } catch (error) {
    unlistenRequest();
    throw error;
  }
  return () => {
    unlistenRequest();
    unlistenClosed?.();
  };
}
