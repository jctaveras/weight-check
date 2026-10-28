import { describe, expect, it } from "vitest";

import {
  MAX_COMPARE_RUNS,
  runIds,
  toggleRun,
  validateCompareSearch,
  withBenchmark,
} from "./compareParams";

describe("validateCompareSearch", () => {
  it("keeps the benchmark and dedupes run ids", () => {
    expect(validateCompareSearch({ benchmark: "inference.latency", runs: "a,b,,a" })).toEqual({
      benchmark: "inference.latency",
      runs: "a,b",
    });
  });

  it("drops empty and invalid values", () => {
    expect(validateCompareSearch({ benchmark: "", runs: ",", extra: 1 })).toEqual({
      benchmark: undefined,
      runs: undefined,
    });
    expect(validateCompareSearch({ benchmark: 12, runs: { x: 1 } }).benchmark).toBe("12");
  });

  it("caps the number of runs", () => {
    const ids = Array.from({ length: MAX_COMPARE_RUNS + 2 }, (_, i) => `r${i}`);
    expect(runIds(validateCompareSearch({ runs: ids.join(",") }))).toHaveLength(MAX_COMPARE_RUNS);
  });
});

describe("toggleRun", () => {
  it("adds and removes runs", () => {
    const one = toggleRun({ benchmark: "b" }, "a");
    expect(one).toEqual({ benchmark: "b", runs: "a" });
    expect(toggleRun(one, "c")).toEqual({ benchmark: "b", runs: "a,c" });
    expect(toggleRun(one, "a")).toEqual({ benchmark: "b", runs: undefined });
  });

  it("ignores additions past the cap", () => {
    const full = { runs: Array.from({ length: MAX_COMPARE_RUNS }, (_, i) => `r${i}`).join(",") };
    expect(toggleRun(full, "extra")).toBe(full);
  });
});

describe("withBenchmark", () => {
  it("clears runs only when the benchmark changes", () => {
    const search = { benchmark: "a", runs: "x" };
    expect(withBenchmark(search, "a")).toBe(search);
    expect(withBenchmark(search, "b")).toEqual({ benchmark: "b" });
    expect(withBenchmark(search, "")).toEqual({ benchmark: undefined });
  });
});
