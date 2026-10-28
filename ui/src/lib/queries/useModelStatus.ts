import { useQuery } from "@tanstack/react-query";

import { getModelStatus } from "@/lib/api/models";
import { queryKeys } from "./keys";

export function useModelStatus() {
  return useQuery({
    queryKey: queryKeys.modelStatus,
    queryFn: () => getModelStatus(),
    refetchInterval: 2000,
  });
}
