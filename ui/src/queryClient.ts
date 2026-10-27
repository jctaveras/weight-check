import { QueryClient } from "@tanstack/react-query";

import { ApiError, type ApiErrorCode } from "@/lib/api/client";

const RETRYABLE = new Set<ApiErrorCode>(["internal", "llama_server", "unknown"]);

export function makeQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 1000,
        refetchOnWindowFocus: true,
        // Retry possibly transient core failures once; client-side errors won't change on retry.
        retry: (count, error) =>
          count < 1 && (!(error instanceof ApiError) || RETRYABLE.has(error.code)),
      },
    },
  });
}
