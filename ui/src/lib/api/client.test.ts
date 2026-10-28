import { describe, expect, it } from "vitest";

import { mockInvoke } from "@/test/render";
import { ApiError, call } from "./client";
import { compactFilter, listRuns } from "./runs";

describe("call", () => {
  it("resolves with the command's result", async () => {
    const invoke = mockInvoke({ health: () => ({ status: "ok" }) });
    await expect(call("health")).resolves.toEqual({ status: "ok" });
    expect(invoke).toHaveBeenCalledWith("health", {});
  });

  it("passes args through to the command", async () => {
    const invoke = mockInvoke({ start_run: () => ({ id: "run-1" }) });
    await call("start_run", { request: { benchmark_id: "x" } });
    expect(invoke.mock.calls[0]).toEqual(["start_run", { request: { benchmark_id: "x" } }]);
  });

  it("maps a { code, message } rejection to ApiError", async () => {
    mockInvoke({
      list_runs: () => {
        throw { code: "no_model_loaded", message: "no model is loaded" };
      },
    });
    const err = await call("list_runs").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ code: "no_model_loaded", message: "no model is loaded" });
  });

  it("maps an unrecognized code to unknown", async () => {
    mockInvoke({
      system_snapshot: () => Promise.reject({ code: "teapot", message: "short and stout" }),
    });
    await expect(call("system_snapshot")).rejects.toMatchObject({
      code: "unknown",
      message: "short and stout",
    });
  });

  it("maps a string rejection (argument deserialization) to a validation ApiError", async () => {
    mockInvoke({
      get_run: () => Promise.reject("invalid args `id` for command `get_run`"),
    });
    await expect(call("get_run", { id: 1 })).rejects.toMatchObject({
      code: "validation",
      message: "invalid args `id` for command `get_run`",
    });
  });

  it("maps any other rejection to an unknown ApiError", async () => {
    mockInvoke({ model_status: () => Promise.reject(42) });
    const err = await call("model_status").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ code: "unknown" });
  });

  it("reports a network ApiError outside the desktop app", async () => {
    await expect(call("health")).rejects.toMatchObject({
      code: "network",
      message: expect.stringContaining("pnpm tauri dev"),
    });
  });
});

describe("listRuns", () => {
  it("drops empty and undefined filter values", async () => {
    expect(compactFilter({ limit: 5, category: undefined, benchmark: "" })).toEqual({ limit: 5 });
    const invoke = mockInvoke({ list_runs: () => [] });
    await listRuns({ limit: 5, benchmark: "" });
    expect(invoke.mock.calls[0]).toEqual(["list_runs", { filter: { limit: 5 } }]);
  });
});
