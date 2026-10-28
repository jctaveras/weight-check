import type { JsonValue, Run, RunWithMetrics } from "@/lib/api/types";
import { shortId } from "@/lib/format";

/** Which way a metric improves. Neutral metrics (settings, utilisation) have no best run. */
export type Direction = "higher" | "lower" | "neutral";

const HIGHER_UNITS = new Set(["tok/s", "req/s", "tok/J"]);
const LOWER_UNITS = new Set(["ms", "MiB", "W", "J"]);
/** Metrics that echo the run's settings rather than measure the model. */
const NEUTRAL_NAMES = new Set(["concurrency", "gpu_layers_offloaded", "offload_pct"]);
/** Summary statistics that clutter the overview; the table still lists them. */
const NOISY_SUFFIXES = ["_min", "_max", "_stddev", "_p95"];

export function metricDirection(name: string, unit: string): Direction {
  if (NEUTRAL_NAMES.has(name)) return "neutral";
  // Spread is worse whatever the unit.
  if (name.endsWith("_stddev")) return "lower";
  if (HIGHER_UNITS.has(unit)) return "higher";
  if (LOWER_UNITS.has(unit)) return "lower";
  return "neutral";
}

export interface Series {
  runId: string;
  label: string;
  /** CSS color for the run's marks, in fixed order so a run keeps its color. */
  color: string;
}

export interface MetricRow {
  name: string;
  unit: string;
  direction: Direction;
  /** One value per series, in series order; undefined when that run didn't report it. */
  values: (number | undefined)[];
  /** Index of the best series, for directional metrics with at least one value. */
  best?: number;
}

export interface Comparison {
  series: Series[];
  metrics: MetricRow[];
}

export const seriesColor = (index: number) => `var(--series-${(index % 8) + 1})`;

function asObject(value: JsonValue | undefined): Record<string, JsonValue> {
  return value && typeof value === "object" && !Array.isArray(value) ? value : {};
}

/** "ctx 8192 · ngl 99" from the llama-server settings stored in the run's config. */
export function settingsLabel(run: Pick<Run, "config">): string {
  const server = asObject(asObject(run.config).server);
  const parts = [
    typeof server.ctx_size === "number" ? `ctx ${server.ctx_size}` : null,
    typeof server.n_gpu_layers === "number" ? `ngl ${server.n_gpu_layers}` : null,
  ];
  return parts.filter(Boolean).join(" · ");
}

/** Labels runs by model, adding settings and then the short id until every label is unique. */
function labels(runs: RunWithMetrics[], modelName: (id: string) => string): string[] {
  const unique = (list: string[]) => list.filter((l) => list.indexOf(l) !== list.lastIndexOf(l));
  let result = runs.map((r) => modelName(r.model_id));
  const clashing = new Set(unique(result));
  result = result.map((l, i) => {
    const settings = settingsLabel(runs[i]);
    return clashing.has(l) && settings ? `${l} · ${settings}` : l;
  });
  const stillClashing = new Set(unique(result));
  return result.map((l, i) => (stillClashing.has(l) ? `${l} · ${shortId(runs[i].id)}` : l));
}

function bestIndex(values: (number | undefined)[], direction: Direction): number | undefined {
  if (direction === "neutral") return undefined;
  let best: number | undefined;
  values.forEach((v, i) => {
    if (v === undefined) return;
    const current = best === undefined ? undefined : values[best]!;
    if (current === undefined || (direction === "higher" ? v > current : v < current)) best = i;
  });
  return best;
}

/** Lines up the metrics of `runs`, in the first run's order (benchmarks emit headline metrics first). */
export function buildComparison(
  runs: RunWithMetrics[],
  modelName: (id: string) => string,
): Comparison {
  const runLabels = labels(runs, modelName);
  const series = runs.map((r, i) => ({ runId: r.id, label: runLabels[i], color: seriesColor(i) }));

  const units = new Map<string, string>();
  for (const run of runs) {
    for (const m of run.metrics) if (!units.has(m.name)) units.set(m.name, m.unit);
  }
  const metrics = [...units].map(([name, unit]) => {
    const values = runs.map((r) => r.metrics.find((m) => m.name === name)?.value);
    const direction = metricDirection(name, unit);
    return { name, unit, direction, values, best: bestIndex(values, direction) };
  });
  return { series, metrics };
}

/**
 * `value` as a percentage of the best value, so the best run scores 100 whichever way the metric
 * improves. Undefined when there is nothing meaningful to scale against (no value, zero or negative).
 */
export function relativeToBest(
  value: number | undefined,
  best: number | undefined,
  direction: Direction,
): number | undefined {
  if (value === undefined || best === undefined || direction === "neutral") return undefined;
  if (value === best) return 100;
  if (direction === "higher") return best > 0 && value >= 0 ? (value / best) * 100 : undefined;
  return best >= 0 && value > 0 ? (best / value) * 100 : undefined;
}

/** Signed difference from the best value in percent, e.g. -12.5 for 12.5% below it. */
export function deltaFromBest(value: number, best: number): number | undefined {
  return best === 0 ? undefined : ((value - best) / Math.abs(best)) * 100;
}

/** Directional metrics worth an overview bar: summary noise dropped, at most `max`. */
export function headlineMetrics(metrics: MetricRow[], max = 6): MetricRow[] {
  return metrics
    .filter((m) => m.best !== undefined && !NOISY_SUFFIXES.some((s) => m.name.endsWith(s)))
    .slice(0, max);
}
