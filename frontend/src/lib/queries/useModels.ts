import { useQuery } from "@tanstack/react-query";

import { listModels } from "@/lib/api/models";
import { useHydratedQuery } from "./hydration";
import { queryKeys } from "./keys";

export function useModels() {
  return useHydratedQuery(
    useQuery({
      queryKey: queryKeys.models,
      queryFn: ({ signal }) => listModels(signal),
      // Listing rescans MODELS_DIR and re-reads GGUF headers; "Rescan" refetches on demand.
      staleTime: 60_000,
    }),
  );
}
