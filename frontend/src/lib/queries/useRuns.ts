import { useQuery } from "@tanstack/react-query";

import { listRuns } from "@/lib/api/runs";
import type { RunFilter, RunStatus } from "@/lib/api/types";
import { useHydratedQuery } from "./hydration";
import { queryKeys } from "./keys";

export const isActive = (status: RunStatus) => status === "queued" || status === "running";

export function useRuns(filter: RunFilter = {}) {
  return useHydratedQuery(
    useQuery({
      queryKey: queryKeys.runs(filter),
      queryFn: ({ signal }) => listRuns(filter, signal),
      refetchInterval: (query) =>
        query.state.data?.some((run) => isActive(run.status)) ? 3000 : false,
    }),
  );
}
