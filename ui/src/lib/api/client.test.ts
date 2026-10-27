import { describe, expect, it, vi } from "vitest";

import { jsonResponse, mockFetch } from "@/test/render";
import { API_URL, ApiError, apiFetch, buildUrl } from "./client";

describe("buildUrl", () => {
  it("drops empty and undefined query values", () => {
    const url = new URL(buildUrl("/runs", { limit: 5, category: undefined, benchmark: "" }));
    expect(url.pathname).toBe("/runs");
    expect([...url.searchParams]).toEqual([["limit", "5"]]);
  });
});

describe("apiFetch", () => {
  it("returns parsed JSON on success", async () => {
    mockFetch(() => jsonResponse({ status: "ok" }));
    await expect(apiFetch("/health")).resolves.toEqual({ status: "ok" });
  });

  it("sends JSON bodies with a content type", async () => {
    const fetch = mockFetch(() => jsonResponse({}, 202));
    await apiFetch("/runs", { method: "POST", body: { benchmark_id: "x" } });
    const [url, init] = fetch.mock.calls[0];
    expect(url).toBe(`${API_URL}/runs`);
    expect(init?.method).toBe("POST");
    expect(init?.headers).toEqual({ "Content-Type": "application/json" });
    expect(init?.body).toBe('{"benchmark_id":"x"}');
  });

  it("returns undefined for 204", async () => {
    mockFetch(() => new Response(null, { status: 204 }));
    await expect(apiFetch("/models/unload", { method: "POST" })).resolves.toBeUndefined();
  });

  it("maps the error envelope to ApiError", async () => {
    mockFetch(() =>
      jsonResponse({ error: { code: "no_model_loaded", message: "no model is loaded" } }, 409),
    );
    const err = await apiFetch("/runs").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ status: 409, code: "no_model_loaded", message: "no model is loaded" });
  });

  it("falls back to the raw text for non-JSON errors", async () => {
    mockFetch(() => new Response("Bad Gateway", { status: 502 }));
    await expect(apiFetch("/system")).rejects.toMatchObject({
      status: 502,
      code: "unknown",
      message: "Bad Gateway",
    });
  });

  it("reports network failures as a network ApiError", async () => {
    vi.stubGlobal("fetch", vi.fn(() => Promise.reject(new TypeError("Failed to fetch"))));
    await expect(apiFetch("/system")).rejects.toMatchObject({ status: 0, code: "network" });
  });
});
