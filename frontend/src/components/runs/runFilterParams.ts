import type { Category, RunFilter } from "@/lib/api/types";

export const DEFAULT_LIMIT = 50;

const CATEGORIES: readonly Category[] = [
  "inference_performance",
  "resource_efficiency",
  "model_quality",
  "agentic",
  "context_scalability",
];

/** Reads run filters from the URL so they survive reloads and can be shared. */
export function filterFromSearchParams(params: URLSearchParams): RunFilter {
  const category = params.get("category");
  const limit = Number(params.get("limit"));
  return {
    model_id: params.get("model_id") || undefined,
    category: CATEGORIES.includes(category as Category) ? (category as Category) : undefined,
    benchmark: params.get("benchmark") || undefined,
    limit: Number.isInteger(limit) && limit > 0 ? limit : DEFAULT_LIMIT,
  };
}

export function searchParamsWith(params: URLSearchParams, key: keyof RunFilter, value: string) {
  const next = new URLSearchParams(params);
  if (value) next.set(key, value);
  else next.delete(key);
  return next;
}
