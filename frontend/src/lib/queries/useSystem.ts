import { useQuery } from "@tanstack/react-query";

import { getSystem } from "@/lib/api/system";
import { useHydratedQuery } from "./hydration";
import { queryKeys } from "./keys";

export function useSystem() {
  return useHydratedQuery(
    useQuery({
      queryKey: queryKeys.system,
      queryFn: ({ signal }) => getSystem(signal),
      refetchInterval: 2000,
    }),
  );
}
