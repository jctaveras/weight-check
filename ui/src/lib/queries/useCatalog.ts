import { useQuery } from "@tanstack/react-query";

import { getCatalog } from "@/lib/api/benchmarks";
import { queryKeys } from "./keys";

export function useCatalog() {
  return useQuery({
    queryKey: queryKeys.catalog,
    queryFn: ({ signal }) => getCatalog(signal),
    staleTime: Infinity,
  });
}
