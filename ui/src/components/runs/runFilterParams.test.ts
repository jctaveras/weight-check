import { describe, expect, it } from "vitest";

import { DEFAULT_LIMIT, filterFromSearch, searchWith, validateRunSearch } from "./runFilterParams";

describe("validateRunSearch", () => {
  it("ignores unknown categories and bad limits", () => {
    expect(validateRunSearch({ category: "nope", limit: "abc", benchmark: "x" })).toEqual({
      model_id: undefined,
      category: undefined,
      benchmark: "x",
      limit: undefined,
    });
    expect(validateRunSearch({ limit: -3 }).limit).toBeUndefined();
    expect(validateRunSearch({ limit: 2.5 }).limit).toBeUndefined();
  });

  it("keeps valid values", () => {
    const search = validateRunSearch({ category: "agentic", limit: 10, model_id: "m" });
    expect(search).toMatchObject({ category: "agentic", limit: 10, model_id: "m" });
  });

  it("accepts values the router parsed as numbers or strings", () => {
    expect(validateRunSearch({ benchmark: 123, limit: "10" })).toMatchObject({ benchmark: "123", limit: 10 });
  });

  it("drops empty and non-scalar values", () => {
    expect(validateRunSearch({ model_id: "", benchmark: { a: 1 }, category: ["agentic"] })).toEqual({
      model_id: undefined,
      category: undefined,
      benchmark: undefined,
      limit: undefined,
    });
  });
});

describe("filterFromSearch", () => {
  it("defaults the limit", () => {
    expect(filterFromSearch({ benchmark: "x" })).toEqual({ benchmark: "x", limit: DEFAULT_LIMIT });
    expect(filterFromSearch({ limit: 5 }).limit).toBe(5);
  });
});

describe("searchWith", () => {
  it("sets and clears keys without mutating the input", () => {
    const search = { benchmark: "a" };
    expect(searchWith(search, "category", "agentic")).toMatchObject({ benchmark: "a", category: "agentic" });
    expect(searchWith(search, "benchmark", "").benchmark).toBeUndefined();
    expect(search).toEqual({ benchmark: "a" });
  });
});
