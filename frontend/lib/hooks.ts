"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";
import { api } from "./api";
import type { Category, SyncStatus } from "./types";

export function useCategories() {
  return useQuery({ queryKey: ["categories"], queryFn: () => api<Category[]>("/categories") });
}

/** Status sinkron + pemicu manual. Selesai sinkron = semua data dimuat ulang. */
export function useSync() {
  const qc = useQueryClient();
  const status = useQuery({
    queryKey: ["sync"],
    queryFn: () => api<SyncStatus>("/sync/status"),
    refetchInterval: (q) => (q.state.data?.status === "syncing" ? 1500 : false),
  });

  const current = status.data?.status;
  const previous = useRef(current);
  useEffect(() => {
    if (previous.current === "syncing" && current && current !== "syncing") {
      qc.invalidateQueries();
    }
    previous.current = current;
  }, [current, qc]);

  const start = useMutation({
    mutationFn: () => api("/sync", { method: "POST" }),
    // Backend membalas 202 lalu bekerja di latar belakang: tandai "syncing" agar polling mulai.
    onSuccess: () =>
      qc.setQueryData<SyncStatus>(["sync"], (d) => d && { ...d, status: "syncing" }),
  });

  return { status: status.data, start, syncing: current === "syncing" || start.isPending };
}
