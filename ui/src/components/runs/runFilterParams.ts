import type { Category, RunFilter } from "@/lib/api/types";

export const DEFAULT_LIMIT = 50;

const CATEGORIES: readonly Category[] = [
  "inference_performance",
  "resource_efficiency",
  "model_quality",
  "agentic",
  "context_scalability",
];

/** The `/runs` search params. Every key is optional so links to `/runs` need no search. */
export interface RunSearch {
  model_id?: string;
  category?: Category;
  benchmark?: string;
  limit?: number;
}

/** The router JSON-parses search values, so `?benchmark=123` arrives as a number. */
function asString(value: unknown): string | undefined {
  if (typeof value === "string") return value || undefined;
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  return undefined;
}

/**
 * `validateSearch` for `/runs`: reads run filters from the URL so they survive reloads and can be
 * shared. Unknown categories and bad limits are dropped.
 */
export function validateRunSearch(search: Record<string, unknown>): RunSearch {
  const category = asString(search.category);
  const limit = Number(search.limit);
  return {
    model_id: asString(search.model_id),
    category: CATEGORIES.includes(category as Category) ? (category as Category) : undefined,
    benchmark: asString(search.benchmark),
    limit: Number.isInteger(limit) && limit > 0 ? limit : undefined,
  };
}

export function filterFromSearch(search: RunSearch): RunFilter {
  return { ...search, limit: search.limit ?? DEFAULT_LIMIT };
}

/** Returns `search` with `key` set, or removed when `value` is empty. */
export function searchWith(search: RunSearch, key: keyof RunFilter, value: string): RunSearch {
  return validateRunSearch({ ...search, [key]: value || undefined });
}
