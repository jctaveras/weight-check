import { useLocation } from "@tanstack/react-router";
import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import type { CategoryCatalog, ModelInfo, Run, RunWithMetrics } from "@/lib/api/types";
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

function run(id: string, modelId: string, tps: number): RunWithMetrics {
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
    created_at: "2026-10-28T10:00:00Z",
    metrics: [{ name: "generation_tps_mean", value: tps, unit: "tok/s" }],
  };
}

const runs = [run("run-a", "m1", 40), run("run-b", "m2", 50)];
const queued: Run = { ...runs[0], id: "run-q", status: "queued" };

function setup() {
  return mockInvoke({
    benchmark_catalog: () => catalog,
    list_models: () => [model("m1", "llama"), model("m2", "qwen")],
    list_runs: () => [...runs, queued],
    compare_runs: ({ ids }) => (ids as string[]).map((id) => runs.find((r) => r.id === id)),
  });
}

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
