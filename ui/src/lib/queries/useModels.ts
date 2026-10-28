import { useQuery } from "@tanstack/react-query";

import { listModels } from "@/lib/api/models";
import { queryKeys } from "./keys";

export function useModels() {
  return useQuery({
    queryKey: queryKeys.models,
    queryFn: () => listModels(),
    // Listing rescans MODELS_DIR and re-reads GGUF headers; "Rescan" refetches on demand.
    staleTime: 60_000,
  });
}
