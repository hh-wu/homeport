import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { ipc } from "@/lib/ipc";
import type { McpConfigInput } from "@/types/models";

export const mcpKeys = {
  state: ["mcp", "state"] as const,
  tokens: ["mcp", "tokens"] as const,
};

export function useMcpState() {
  return useQuery({ queryKey: mcpKeys.state, queryFn: ipc.mcp.getState });
}

export function useSetMcpConfig() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: McpConfigInput) => ipc.mcp.setConfig(input),
    onSuccess: (state) => qc.setQueryData(mcpKeys.state, state),
  });
}

export function useMcpTokens() {
  return useQuery({ queryKey: mcpKeys.tokens, queryFn: ipc.mcp.listTokens });
}

export function useGenerateMcpToken() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (name: string) => ipc.mcp.generateToken(name),
    onSuccess: () => qc.invalidateQueries({ queryKey: mcpKeys.tokens }),
  });
}

export function useRevokeMcpToken() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (name: string) => ipc.mcp.revokeToken(name),
    onSuccess: () => qc.invalidateQueries({ queryKey: mcpKeys.tokens }),
  });
}
