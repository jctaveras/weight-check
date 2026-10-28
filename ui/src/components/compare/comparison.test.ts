import { describe, expect, it } from "vitest";

import type { Metric, RunWithMetrics } from "@/lib/api/types";
import {
  buildComparison,
  deltaFromBest,
  headlineMetrics,
  metricDirection,
  relativeToBest,
} from "./comparison";

function run(id: string, modelId: string, metrics: Metric[], server = {}): RunWithMetrics {
  return {
    id,
    model_id: modelId,
    hardware_id: "hw",
    category: "inference_performance",
    benchmark_name: "inference.generation_speed",
    status: "completed",
    config: { benchmark: {}, server },
    details: null,
    error: null,
    started_at: null,
    finished_at: null,
    created_at: "2026-10-28T10:00:00Z",
    metrics,
  };
}

const tps = (value: number): Metric => ({ name: "generation_tps_mean", value, unit: "tok/s" });
const names: Record<string, string> = { m1: "llama", m2: "qwen" };
const modelName = (id: string) => names[id] ?? id;

describe("metricDirection", () => {
  it.each([
    ["generation_tps_mean", "tok/s", "higher"],
    ["tokens_per_joule", "tok/J", "higher"],
    ["ttft_ms_median", "ms", "lower"],
    ["vram_peak_mb", "MiB", "lower"],
    ["power_avg_w", "W", "lower"],
    ["generation_tps_stddev", "tok/s", "lower"],
    ["concurrency", "clients", "neutral"],
    ["offload_pct", "%", "neutral"],
    ["gpu_util_avg_pct", "%", "neutral"],
  ])("%s (%s) is %s", (name, unit, expected) => {
    expect(metricDirection(name, unit)).toBe(expected);
  });
});

describe("buildComparison", () => {
  it("aligns metrics across runs and picks the best", () => {
    const { series, metrics } = buildComparison(
      [
        run("a", "m1", [tps(40), { name: "ttft_ms_median", value: 90, unit: "ms" }]),
        run("b", "m2", [tps(55), { name: "ttft_ms_median", value: 120, unit: "ms" }]),
        run("c", "m3", [{ name: "concurrency", value: 4, unit: "clients" }]),
      ],
      modelName,
    );
    expect(series.map((s) => s.label)).toEqual(["llama", "qwen", "m3"]);
    expect(series.map((s) => s.color)).toEqual(["var(--series-1)", "var(--series-2)", "var(--series-3)"]);
    expect(metrics.map((m) => m.name)).toEqual(["generation_tps_mean", "ttft_ms_median", "concurrency"]);
    expect(metrics[0]).toMatchObject({ values: [40, 55, undefined], best: 1 });
    expect(metrics[1]).toMatchObject({ direction: "lower", best: 0 });
    expect(metrics[2].best).toBeUndefined();
  });

  it("disambiguates runs of the same model by settings, then by id", () => {
    const { series } = buildComparison(
      [
        run("aaaaaaaa-1", "m1", [], { ctx_size: 4096, n_gpu_layers: 20 }),
        run("bbbbbbbb-2", "m1", [], { ctx_size: 8192, n_gpu_layers: 99 }),
        run("cccccccc-3", "m1", [], { ctx_size: 8192, n_gpu_layers: 99 }),
        run("dddddddd-4", "m2", []),
      ],
      modelName,
    );
    expect(series.map((s) => s.label)).toEqual([
      "llama · ctx 4096 · ngl 20",
      "llama · ctx 8192 · ngl 99 · bbbbbbbb",
      "llama · ctx 8192 · ngl 99 · cccccccc",
      "qwen",
    ]);
  });
});

describe("relativeToBest", () => {
  it("scores the best run 100 in either direction", () => {
    expect(relativeToBest(50, 100, "higher")).toBe(50);
    expect(relativeToBest(200, 100, "lower")).toBe(50);
    expect(relativeToBest(100, 100, "lower")).toBe(100);
  });

  it("is undefined without a meaningful scale", () => {
    expect(relativeToBest(undefined, 100, "higher")).toBeUndefined();
    expect(relativeToBest(5, 10, "neutral")).toBeUndefined();
    expect(relativeToBest(0, 0, "higher")).toBe(100);
    expect(relativeToBest(5, 0, "lower")).toBe(0);
    expect(relativeToBest(0, -1, "lower")).toBeUndefined();
  });
});

it("deltaFromBest is signed and guards zero", () => {
  expect(deltaFromBest(90, 100)).toBeCloseTo(-10);
  expect(deltaFromBest(120, 100)).toBeCloseTo(20);
  expect(deltaFromBest(1, 0)).toBeUndefined();
});

it("headlineMetrics keeps directional, non-noisy metrics", () => {
  const { metrics } = buildComparison(
    [
      run("a", "m1", [
        tps(40),
        { name: "generation_tps_p95", value: 41, unit: "tok/s" },
        { name: "concurrency", value: 1, unit: "clients" },
        { name: "ttft_ms_median", value: 9, unit: "ms" },
      ]),
    ],
    modelName,
  );
  expect(headlineMetrics(metrics).map((m) => m.name)).toEqual(["generation_tps_mean", "ttft_ms_median"]);
  expect(headlineMetrics(metrics, 1)).toHaveLength(1);
});
