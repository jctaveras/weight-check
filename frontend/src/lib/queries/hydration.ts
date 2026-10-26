import type { UseQueryResult } from "@tanstack/react-query";
import { useSyncExternalStore } from "react";

const noopSubscribe = () => () => {};

/** False during SSR and the hydration render, true afterwards. */
export function useHydrated(): boolean {
  return useSyncExternalStore(
    noopSubscribe,
    () => true,
    () => false,
  );
}

/**
 * Reports a query as pending until hydration completes. Another component can fill the
 * cache before a later Suspense boundary hydrates, which would otherwise render data the
 * server never saw and cause a hydration mismatch.
 */
export function useHydratedQuery<TData, TError>(
  result: UseQueryResult<TData, TError>,
): UseQueryResult<TData, TError> {
  const hydrated = useHydrated();
  if (hydrated) return result;
  return {
    ...result,
    data: undefined,
    error: null,
    status: "pending",
    isPending: true,
    isLoading: true,
    isSuccess: false,
    isError: false,
    isLoadingError: false,
    isRefetchError: false,
  } as UseQueryResult<TData, TError>;
}
