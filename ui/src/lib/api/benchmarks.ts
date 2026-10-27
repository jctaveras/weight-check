import { apiFetch } from "./client";
import type { CategoryCatalog } from "./types";

export const getCatalog = (signal?: AbortSignal) =>
  apiFetch<CategoryCatalog[]>("/benchmarks", { signal });
