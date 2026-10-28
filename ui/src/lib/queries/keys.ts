import type { RunFilter } from "@/lib/api/types";

export const queryKeys = {
  system: ["system"] as const,
  models: ["models"] as const,
  modelStatus: ["models", "status"] as const,
  catalog: ["benchmarks"] as const,
  runs: (filter: RunFilter = {}) => ["runs", "list", filter] as const,
  runPages: (filter: RunFilter = {}) => ["runs", "pages", filter] as const,
  runsAll: ["runs"] as const,
  run: (id: string) => ["runs", "detail", id] as const,
  compare: (ids: string[]) => ["runs", "compare", ids] as const,
};
