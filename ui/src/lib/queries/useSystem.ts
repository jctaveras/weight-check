import { useQuery } from "@tanstack/react-query";

import { getSystem } from "@/lib/api/system";
import { queryKeys } from "./keys";

export function useSystem() {
  return useQuery({
    queryKey: queryKeys.system,
    queryFn: () => getSystem(),
    refetchInterval: 2000,
  });
}
