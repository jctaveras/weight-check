import { useQuery } from "@tanstack/react-query";

import { getModelStatus } from "@/lib/api/models";
import { useHydratedQuery } from "./hydration";
import { queryKeys } from "./keys";

export function useModelStatus() {
  return useHydratedQuery(
    useQuery({
      queryKey: queryKeys.modelStatus,
      queryFn: ({ signal }) => getModelStatus(signal),
      refetchInterval: 2000,
    }),
  );
}
