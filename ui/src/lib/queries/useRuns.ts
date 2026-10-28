import { useQuery } from "@tanstack/react-query";

import { listRuns } from "@/lib/api/runs";
import type { RunFilter, RunStatus } from "@/lib/api/types";
import { queryKeys } from "./keys";

export const isActive = (status: RunStatus) => status === "queued" || status === "running";

export function useRuns(filter: RunFilter = {}) {
  return useQuery({
    queryKey: queryKeys.runs(filter),
    queryFn: () => listRuns(filter),
    refetchInterval: (query) =>
      query.state.data?.some((run) => isActive(run.status)) ? 3000 : false,
  });
}
