import { apiFetch } from "./client";
import type { Run, RunFilter, RunWithMetrics, StartRunRequest } from "./types";

export const listRuns = (filter: RunFilter = {}, signal?: AbortSignal) =>
  apiFetch<Run[]>("/runs", { query: { ...filter }, signal });

export const getRun = (id: string, signal?: AbortSignal) =>
  apiFetch<RunWithMetrics>(`/runs/${encodeURIComponent(id)}`, { signal });

export const startRun = (body: StartRunRequest) =>
  apiFetch<Run>("/runs", { method: "POST", body });
