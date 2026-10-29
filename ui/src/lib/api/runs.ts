import { call } from "./client";
import type { Run, RunFilter, RunWithMetrics, StartRunRequest } from "./types";

/** Drops undefined and empty-string values so the core only sees filters that are set. */
export function compactFilter(filter: RunFilter): RunFilter {
  return Object.fromEntries(
    Object.entries(filter).filter(([, value]) => value !== undefined && value !== ""),
  ) as RunFilter;
}

export const listRuns = (filter: RunFilter = {}) =>
  call<Run[]>("list_runs", { filter: compactFilter(filter) });

export const getRun = (id: string) => call<RunWithMetrics>("get_run", { id });

export const startRun = (request: StartRunRequest) => call<Run>("start_run", { request });

/** Asks a queued or running run to stop; it turns `cancelled` once it has wound down. */
export const cancelRun = (id: string) => call<Run>("cancel_run", { id });

/** Deletes a finished run and its metrics. */
export const deleteRun = (id: string) => call<null>("delete_run", { id });

/** Completed runs of one benchmark with their metrics, in the order of `ids`. */
export const compareRuns = (ids: string[]) => call<RunWithMetrics[]>("compare_runs", { ids });
