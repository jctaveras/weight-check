import { useQueries } from "@tanstack/react-query";

import { getRun } from "@/lib/api/runs";
import { queryKeys } from "./keys";

/** Fetches each run in `ids` on its own, e.g. picked runs that aren't on a loaded list page. */
export function useRunsById(ids: string[]) {
  return useQueries({
    queries: ids.map((id) => ({ queryKey: queryKeys.run(id), queryFn: () => getRun(id) })),
  });
}
