import { useQuery } from "@tanstack/react-query";

import { getRun } from "@/lib/api/runs";
import { queryKeys } from "./keys";
import { isActive } from "./useRuns";

export function useRun(id: string) {
  return useQuery({
    queryKey: queryKeys.run(id),
    queryFn: ({ signal }) => getRun(id, signal),
    refetchInterval: (query) => {
      const status = query.state.data?.status;
      return status === undefined || isActive(status) ? 1000 : false;
    },
  });
}
