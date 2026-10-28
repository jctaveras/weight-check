import { keepPreviousData, useQuery } from "@tanstack/react-query";

import { compareRuns } from "@/lib/api/runs";
import { queryKeys } from "./keys";

export function useCompareRuns(ids: string[]) {
  return useQuery({
    queryKey: queryKeys.compare(ids),
    queryFn: () => compareRuns(ids),
    enabled: ids.length > 0,
    // Keep the last comparison on screen while a toggled run loads.
    placeholderData: keepPreviousData,
  });
}
