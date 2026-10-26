import { useQuery } from "@tanstack/react-query";

import { getCatalog } from "@/lib/api/benchmarks";
import { useHydratedQuery } from "./hydration";
import { queryKeys } from "./keys";

export function useCatalog() {
  return useHydratedQuery(
    useQuery({
      queryKey: queryKeys.catalog,
      queryFn: ({ signal }) => getCatalog(signal),
      staleTime: Infinity,
    }),
  );
}
