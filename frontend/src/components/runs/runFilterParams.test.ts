import { describe, expect, it } from "vitest";

import { DEFAULT_LIMIT, filterFromSearchParams, searchParamsWith } from "./runFilterParams";

describe("filterFromSearchParams", () => {
  it("ignores unknown categories and bad limits", () => {
    const filter = filterFromSearchParams(new URLSearchParams("category=nope&limit=abc&benchmark=x"));
    expect(filter).toEqual({
      model_id: undefined,
      category: undefined,
      benchmark: "x",
      limit: DEFAULT_LIMIT,
    });
  });

  it("keeps valid values", () => {
    const filter = filterFromSearchParams(new URLSearchParams("category=agentic&limit=10"));
    expect(filter.category).toBe("agentic");
    expect(filter.limit).toBe(10);
  });
});

describe("searchParamsWith", () => {
  it("sets and clears keys without mutating the input", () => {
    const params = new URLSearchParams("benchmark=a");
    expect(searchParamsWith(params, "category", "agentic").toString()).toBe("benchmark=a&category=agentic");
    expect(searchParamsWith(params, "benchmark", "").toString()).toBe("");
    expect(params.toString()).toBe("benchmark=a");
  });
});
