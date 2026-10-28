import { useLocation } from "@tanstack/react-router";
import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import type { CategoryCatalog, ModelInfo, Run, RunFilter, RunWithMetrics } from "@/lib/api/types";
import { RUN_PAGE_SIZE } from "@/lib/queries/useRunPages";
import { mockInvoke, renderWithRouter } from "@/test/render";
import { CompareView } from "./CompareView";
import { validateCompareSearch } from "./compareParams";

const catalog: CategoryCatalog[] = [
  {
    category: "inference_performance",
    title: "Inference Performance",
    description: "",
    benchmarks: [
      {
        id: "inference.generation_speed",
        category: "inference_performance",
        name: "Generation Speed",
        description: "",
        implemented: true,
        default_config: {},
      },
    ],
  },
];

const model = (id: string, name: string) => ({ id, name }) as ModelInfo;

function run(id: string, modelId: string, tps: number, createdAt = "2026-10-28T10:00:00Z"): RunWithMetrics {
  return {
    id,
    model_id: modelId,
    hardware_id: "hw",
    category: "inference_performance",
    benchmark_name: "inference.generation_speed",
    status: "completed",
    config: { benchmark: {}, server: { ctx_size: 4096, n_gpu_layers: 20 } },
    details: null,
    error: null,
    started_at: null,
    finished_at: null,
    created_at: createdAt,
    metrics: [{ name: "generation_tps_mean", value: tps, unit: "tok/s" }],
  };
}

const runs = [run("run-a", "m1", 40), run("run-b", "m2", 50)];
const queued: Run = { ...runs[0], id: "run-q", status: "queued" };

/** Stands in for the core's `list_runs`: filters `all` (newest first) by status, then pages it. */
function listRuns(all: Run[]) {
  return ({ filter }: Record<string, unknown>) => {
    const { status, limit = 100, offset = 0 } = filter as RunFilter;
    return all.filter((r) => !status || r.status === status).slice(offset, offset + limit);
  };
}

function setup(all: Run[] = [...runs, queued], extra: Parameters<typeof mockInvoke>[0] = {}) {
  return mockInvoke({
    benchmark_catalog: () => catalog,
    list_models: () => [model("m1", "llama"), model("m2", "qwen")],
    list_runs: listRuns(all),
    compare_runs: ({ ids }) => (ids as string[]).map((id) => runs.find((r) => r.id === id)),
    ...extra,
  });
}

/** `count` runs of `status`, all newer than `runs`. */
const newer = (count: number, status: Run["status"]): Run[] =>
  Array.from({ length: count }, (_, i) => ({
    ...runs[0],
    id: `new-${i}`,
    status,
    created_at: "2026-10-28T12:00:00Z",
  }));

const historical = run("run-old", "m2", 30, "2025-01-01T10:00:00Z");

/** Renders the view the way the route does, reading the search from the test router's URL. */
function Routed() {
  const search = useLocation({ select: (l) => l.search });
  return <CompareView search={validateCompareSearch(search)} />;
}

const renderAt = (path: string) => renderWithRouter(<Routed />, { path });

describe("CompareView", () => {
  it("asks for a benchmark first", async () => {
    setup();
    await renderAt("/compare");
    expect(screen.getByText("Choose a Benchmark")).toBeInTheDocument();
  });

  it("lists completed runs and puts picked ones in the URL", async () => {
    setup();
    const user = userEvent.setup();
    const { router } = await renderAt("/compare?benchmark=inference.generation_speed");

    const boxes = await screen.findAllByRole("checkbox");
    expect(boxes).toHaveLength(2); // the queued run isn't offered
    expect(screen.getByText("Pick at Least Two Runs")).toBeInTheDocument();

    await user.click(boxes[0]);
    expect(router.state.location.search).toMatchObject({ runs: "run-a" });
  });

  it("asks the core for completed runs only, so failed runs can't crowd them out", async () => {
    const invoke = setup([...newer(RUN_PAGE_SIZE, "failed"), historical]);
    await renderAt("/compare?benchmark=inference.generation_speed");

    const boxes = await screen.findAllByRole("checkbox");
    expect(boxes).toHaveLength(1);
    expect(boxes[0]).toHaveAccessibleName(/qwen run from .*2025/);
    expect(screen.queryByText("No Completed Runs")).not.toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("list_runs", {
      filter: {
        benchmark: "inference.generation_speed",
        status: "completed",
        limit: RUN_PAGE_SIZE,
        offset: 0,
      },
    });
  });

  it("loads older completed runs a page at a time", async () => {
    const invoke = setup([...newer(RUN_PAGE_SIZE, "completed"), historical]);
    const user = userEvent.setup();
    await renderAt("/compare?benchmark=inference.generation_speed");

    expect(await screen.findAllByRole("checkbox")).toHaveLength(RUN_PAGE_SIZE);
    await user.click(screen.getByRole("button", { name: "Load Older Runs" }));

    expect(await screen.findByRole("checkbox", { name: /2025/ })).toBeInTheDocument();
    expect(screen.getAllByRole("checkbox")).toHaveLength(RUN_PAGE_SIZE + 1);
    expect(screen.queryByRole("button", { name: /Older Runs/ })).not.toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("list_runs", {
      filter: expect.objectContaining({ offset: RUN_PAGE_SIZE }),
    });
  });

  it("pins a picked run that isn't on a loaded page so it can be deselected", async () => {
    setup([...newer(RUN_PAGE_SIZE, "completed"), historical], {
      get_run: ({ id }) => (id === historical.id ? historical : Promise.reject({ code: "not_found", message: "no" })),
    });
    const user = userEvent.setup();
    const { router } = await renderAt("/compare?benchmark=inference.generation_speed&runs=run-old");

    const boxes = await screen.findAllByRole("checkbox");
    expect(boxes).toHaveLength(RUN_PAGE_SIZE + 1);
    expect(boxes[0]).toBeChecked();
    expect(boxes[0]).toHaveAccessibleName(/qwen run from .*2025/);

    await user.click(boxes[0]);
    expect(router.state.location.search).not.toHaveProperty("runs");
  });

  it("lets you remove a picked run that can't be loaded", async () => {
    setup(undefined, { get_run: () => Promise.reject({ code: "not_found", message: "run not found" }) });
    const user = userEvent.setup();
    const { router } = await renderAt("/compare?benchmark=inference.generation_speed&runs=gone");

    expect(await screen.findByText(/couldn’t be loaded/)).toHaveTextContent("gone");
    await user.click(screen.getByRole("button", { name: "Remove" }));
    expect(router.state.location.search).not.toHaveProperty("runs");
    expect(screen.queryByText(/couldn’t be loaded/)).not.toBeInTheDocument();
  });

  it("charts and tabulates the picked runs", async () => {
    const invoke = setup();
    await renderAt("/compare?benchmark=inference.generation_speed&runs=run-a,run-b");

    const table = await screen.findByRole("table", { name: "Metrics of the compared runs" });
    const row = within(table).getByRole("row", { name: /generation_tps_mean/ });
    expect(within(row).getByText("Best")).toBeInTheDocument();
    expect(within(row).getByText("-20%")).toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Runs" })).toHaveTextContent(/llama.*qwen/);
    expect(invoke).toHaveBeenCalledWith("compare_runs", { ids: ["run-a", "run-b"] });
  });
});
