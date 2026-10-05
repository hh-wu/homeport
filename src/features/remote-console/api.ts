import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { ipc } from "@/lib/ipc";

export const remoteConsoleKeys = {
  status: ["remoteConsole", "status"] as const,
  log: ["remoteConsole", "log"] as const,
  rustdesk: ["remoteConsole", "rustdesk"] as const,
};

export function useRcStatus() {
  return useQuery({
    queryKey: remoteConsoleKeys.status,
    queryFn: ipc.remoteConsole.status,
    refetchInterval: 3000,
    staleTime: 0,
  });
}

export function useRcLog(lines = 200) {
  return useQuery({
    queryKey: [...remoteConsoleKeys.log, lines],
    queryFn: () => ipc.remoteConsole.logTail(lines),
    refetchInterval: 5000,
    staleTime: 0,
  });
}

export function useRcRustDesk() {
  return useQuery({
    queryKey: remoteConsoleKeys.rustdesk,
    queryFn: ipc.remoteConsole.rustdesk,
    staleTime: Infinity,
  });
}

export function useRcStart() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ipc.remoteConsole.start,
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: remoteConsoleKeys.status,
      });
      void queryClient.invalidateQueries({ queryKey: remoteConsoleKeys.log });
    },
  });
}

export function useRcStop() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ipc.remoteConsole.stop,
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: remoteConsoleKeys.status }),
  });
}

export function useRcSetWatchdog() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (enabled: boolean) => ipc.remoteConsole.setWatchdog(enabled),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: remoteConsoleKeys.status }),
  });
}

export function useRcCheckProxies() {
  return useMutation({ mutationFn: ipc.remoteConsole.checkProxies });
}

export function useRcCloud() {
  return useMutation({ mutationFn: ipc.remoteConsole.cloud });
}
