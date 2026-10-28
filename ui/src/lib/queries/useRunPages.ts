import { useInfiniteQuery } from "@tanstack/react-query";

import { listRuns } from "@/lib/api/runs";
import type { Run, RunFilter } from "@/lib/api/types";
import { queryKeys } from "./keys";

export const RUN_PAGE_SIZE = 100;

/** Runs matching `filter`, newest first, fetched `RUN_PAGE_SIZE` at a time with `fetchNextPage`. */
export function useRunPages(filter: Omit<RunFilter, "limit" | "offset">) {
  return useInfiniteQuery({
    queryKey: queryKeys.runPages(filter),
    queryFn: ({ pageParam }) => listRuns({ ...filter, limit: RUN_PAGE_SIZE, offset: pageParam }),
    initialPageParam: 0,
    // A short page is the last one.
    getNextPageParam: (last: Run[], pages) =>
      last.length < RUN_PAGE_SIZE ? undefined : pages.length * RUN_PAGE_SIZE,
    select: (data) => {
      // Offsets shift when runs finish between page loads, so a run can show up on two pages.
      const seen = new Set<string>();
      return data.pages.flat().filter((run) => !seen.has(run.id) && seen.add(run.id));
    },
  });
}
