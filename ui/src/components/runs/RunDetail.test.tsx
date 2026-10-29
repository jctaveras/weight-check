import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { RunStatus, RunWithMetrics } from "@/lib/api/types";
import { mockInvoke, renderWithRouter } from "@/test/render";
import { RunDetail } from "./RunDetail";

function run(status: RunStatus): RunWithMetrics {
  return {
    id: "run-1",
    model_id: "m1",
    hardware_id: "hw",
    category: "model_quality",
    benchmark_name: "quality.math",
    status,
    config: { benchmark: {}, server: {} },
    details: null,
    error: null,
    started_at: null,
    finished_at: null,
    created_at: "2026-10-29T10:00:00Z",
    metrics: [],
  };
}

const lookups = { benchmark_catalog: () => [], list_models: () => [] };

describe("RunDetail", () => {
  it("cancels an active run and waits for it to wind down", async () => {
    let status: RunStatus = "running";
    const invoke = mockInvoke({
      ...lookups,
      get_run: () => run(status),
      cancel_run: () => run("running"),
    });
    await renderWithRouter(<RunDetail id="run-1" />, { path: "/runs/run-1" });

    await userEvent.click(await screen.findByRole("button", { name: "Cancel Run" }));
    expect(invoke).toHaveBeenCalledWith("cancel_run", { id: "run-1" });
    expect(await screen.findByRole("button", { name: /Cancelling/ })).toBeDisabled();
    expect(screen.queryByRole("button", { name: "Delete Run" })).not.toBeInTheDocument();

    status = "cancelled";
    expect(await screen.findByText("Cancelled", {}, { timeout: 3000 })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Cancel/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Delete Run" })).toBeInTheDocument();
  });

  it("deletes a finished run and returns to the runs list", async () => {
    mockInvoke({ ...lookups, get_run: () => run("completed"), delete_run: () => null });
    const { router } = await renderWithRouter(<RunDetail id="run-1" />, { path: "/runs/run-1" });

    await userEvent.click(await screen.findByRole("button", { name: "Delete Run" }));
    await userEvent.click(screen.getByRole("button", { name: "Confirm Delete" }));
    await vi.waitFor(() => expect(router.state.location.pathname).toBe("/runs"));
  });
});
