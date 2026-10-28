/** Most runs one comparison shows; matches `MAX_COMPARE_RUNS` in the core. */
export const MAX_COMPARE_RUNS = 8;

/** The `/compare` search params. Run ids are kept as one comma-separated string in the URL. */
export interface CompareSearch {
  benchmark?: string;
  runs?: string;
}

/** The router JSON-parses search values, so `?benchmark=123` arrives as a number. */
function asString(value: unknown): string | undefined {
  if (typeof value === "string") return value || undefined;
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  return undefined;
}

/** Splits the `runs` param into unique, non-empty ids, capped at `MAX_COMPARE_RUNS`. */
export function runIds(search: CompareSearch): string[] {
  const ids = (search.runs ?? "").split(",").map((id) => id.trim()).filter(Boolean);
  return [...new Set(ids)].slice(0, MAX_COMPARE_RUNS);
}

function withRuns(search: CompareSearch, ids: string[]): CompareSearch {
  return { benchmark: search.benchmark, runs: ids.length ? ids.join(",") : undefined };
}

/** `validateSearch` for `/compare`: keeps the picked benchmark and runs shareable and reloadable. */
export function validateCompareSearch(search: Record<string, unknown>): CompareSearch {
  const benchmark = asString(search.benchmark);
  return withRuns({ benchmark }, runIds({ runs: asString(search.runs) }));
}

/** Switching benchmarks drops the picked runs; they belong to the previous benchmark. */
export function withBenchmark(search: CompareSearch, benchmark: string): CompareSearch {
  return search.benchmark === benchmark ? search : { benchmark: benchmark || undefined };
}

/** Adds or removes `id`, ignoring additions past `MAX_COMPARE_RUNS`. */
export function toggleRun(search: CompareSearch, id: string): CompareSearch {
  const ids = runIds(search);
  if (ids.includes(id)) return withRuns(search, ids.filter((x) => x !== id));
  return ids.length >= MAX_COMPARE_RUNS ? search : withRuns(search, [...ids, id]);
}
