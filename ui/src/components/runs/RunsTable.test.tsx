import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { Run, RunStatus } from "@/lib/api/types";
import { mockInvoke, renderWithRouter } from "@/test/render";
import { RunsTable } from "./RunsTable";

const run = (id: string, status: RunStatus): Run => ({
  id,
  model_id: "m1",
  hardware_id: "hw",
  category: "model_quality",
  benchmark_name: "quality.math",
  status,
  config: {},
  details: null,
  error: null,
  started_at: null,
  finished_at: null,
  created_at: "2026-10-29T10:00:00Z",
});

describe("RunsTable", () => {
  it("offers Delete Run on finished rows only when deletable", async () => {
    mockInvoke({ benchmark_catalog: () => [], list_models: () => [] });
    const runs = [run("done", "completed"), run("live", "running"), run("stopped", "cancelled")];
    await renderWithRouter(<RunsTable runs={runs} deletable />);

    const rows = screen.getAllByRole("row").slice(1);
    expect(within(rows[0]).getByRole("button", { name: "Delete Run" })).toBeInTheDocument();
    expect(within(rows[1]).queryByRole("button", { name: "Delete Run" })).not.toBeInTheDocument();
    expect(within(rows[2]).getByRole("button", { name: "Delete Run" })).toBeInTheDocument();
  });

  it("has no actions by default", async () => {
    mockInvoke({ benchmark_catalog: () => [], list_models: () => [] });
    await renderWithRouter(<RunsTable runs={[run("done", "completed")]} />);
    expect(screen.queryByRole("button", { name: "Delete Run" })).not.toBeInTheDocument();
  });
});
