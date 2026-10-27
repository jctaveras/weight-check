import { describe, expect, it } from "vitest";

import {
  elapsedMs,
  formatBytes,
  formatDuration,
  formatMetricValue,
  formatPercent,
  humanize,
} from "./format";

const NBSP = " ";

describe("formatBytes", () => {
  it("uses binary units", () => {
    expect(formatBytes(512, "en-US")).toBe(`512${NBSP}B`);
    expect(formatBytes(1536, "en-US")).toBe(`1.5${NBSP}KiB`);
    expect(formatBytes(24 * 1024 ** 3, "en-US")).toBe(`24${NBSP}GiB`);
  });

  it("renders a dash for missing values", () => {
    expect(formatBytes(null)).toBe("—");
    expect(formatBytes(undefined)).toBe("—");
  });
});

describe("formatPercent", () => {
  it("takes a 0–100 scale", () => {
    expect(formatPercent(42.25, "en-US")).toBe("42.3%");
    expect(formatPercent(null)).toBe("—");
  });
});

describe("formatDuration", () => {
  it("picks ms, s or min", () => {
    expect(formatDuration(850, "en-US")).toBe(`850${NBSP}ms`);
    expect(formatDuration(12_340, "en-US")).toBe(`12.3${NBSP}s`);
    expect(formatDuration(125_000, "en-US")).toBe(`2${NBSP}min 5${NBSP}s`);
    expect(formatDuration(null)).toBe("—");
  });
});

describe("elapsedMs", () => {
  it("needs both timestamps", () => {
    expect(elapsedMs("2026-01-01T00:00:00Z", "2026-01-01T00:00:01.5Z")).toBe(1500);
    expect(elapsedMs("2026-01-01T00:00:00Z", null)).toBeNull();
  });
});

describe("formatMetricValue", () => {
  it("keeps fewer decimals for large values", () => {
    expect(formatMetricValue(123.456, "en-US")).toBe("123.5");
    expect(formatMetricValue(12.3456, "en-US")).toBe("12.35");
  });
});

describe("humanize", () => {
  it("title-cases the last id segment", () => {
    expect(humanize("inference.generation_speed")).toBe("Generation Speed");
    expect(humanize("model_quality")).toBe("Model Quality");
  });
});
