import { fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import type { BenchmarkDescriptor } from "@/lib/api/types";
import { jsonResponse, mockFetch, renderWithRouter } from "@/test/render";
import { BenchmarkCard } from "./BenchmarkCard";
import { RunBenchmarkForm } from "./RunBenchmarkForm";

const benchmark: BenchmarkDescriptor = {
  id: "inference.latency",
  category: "inference_performance",
  name: "Latency",
  description: "Time to first token",
  implemented: true,
  default_config: { n_predict: 128, repetitions: 5 },
};

const textarea = () => screen.getByLabelText("Config (JSON)") as HTMLTextAreaElement;

describe("RunBenchmarkForm", () => {
  it("pre-fills the default config", async () => {
    await renderWithRouter(<RunBenchmarkForm benchmark={benchmark} />);
    expect(JSON.parse(textarea().value)).toEqual(benchmark.default_config);
  });

  it("blocks submit on invalid JSON and focuses the editor", async () => {
    const fetch = mockFetch(() => jsonResponse({}));
    await renderWithRouter(<RunBenchmarkForm benchmark={benchmark} />);
    fireEvent.change(textarea(), { target: { value: "{ nope" } });
    await userEvent.click(screen.getByRole("button", { name: "Start Run" }));
    expect(screen.getByText(/Invalid JSON/)).toBeInTheDocument();
    expect(textarea()).toHaveFocus();
    expect(textarea()).toHaveAttribute("aria-invalid", "true");
    expect(fetch).not.toHaveBeenCalled();
  });

  it("rejects non-object JSON", async () => {
    await renderWithRouter(<RunBenchmarkForm benchmark={benchmark} />);
    fireEvent.change(textarea(), { target: { value: "[1, 2]" } });
    await userEvent.click(screen.getByRole("button", { name: "Start Run" }));
    expect(screen.getByText(/must be a JSON object/)).toBeInTheDocument();
  });

  it("resets to defaults", async () => {
    await renderWithRouter(<RunBenchmarkForm benchmark={benchmark} />);
    const reset = screen.getByRole("button", { name: "Reset to Defaults" });
    expect(reset).toBeDisabled();
    fireEvent.change(textarea(), { target: { value: "{}" } });
    await userEvent.click(reset);
    expect(JSON.parse(textarea().value)).toEqual(benchmark.default_config);
  });

  it("starts a run and navigates to it", async () => {
    const fetch = mockFetch(() => jsonResponse({ id: "run-1", status: "queued" }, 202));
    const { router } = await renderWithRouter(<RunBenchmarkForm benchmark={benchmark} />, { path: "/benchmarks" });
    fireEvent.change(textarea(), { target: { value: '{"repetitions": 1}' } });
    await userEvent.click(screen.getByRole("button", { name: "Start Run" }));
    await waitFor(() => expect(router.state.location.pathname).toBe("/runs/run-1"));
    expect(JSON.parse(String(fetch.mock.calls[0][1]?.body))).toEqual({
      benchmark_id: "inference.latency",
      config: { repetitions: 1 },
    });
  });

  it("shows the no-model error with a link to Models", async () => {
    mockFetch(() => jsonResponse({ error: { code: "no_model_loaded", message: "no model is loaded" } }, 409));
    const { router } = await renderWithRouter(<RunBenchmarkForm benchmark={benchmark} />, { path: "/benchmarks" });
    await userEvent.click(screen.getByRole("button", { name: "Start Run" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("No model is loaded.");
    expect(screen.getByRole("link", { name: "Go to Models" })).toHaveAttribute("href", "/models");
    expect(router.state.location.pathname).toBe("/benchmarks");
  });
});

describe("BenchmarkCard", () => {
  it("disables running unimplemented benchmarks", async () => {
    await renderWithRouter(<BenchmarkCard benchmark={{ ...benchmark, implemented: false }} />);
    expect(screen.getByText("Not Implemented")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Start Run" })).toBeDisabled();
    expect(screen.queryByLabelText("Config (JSON)")).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "View past runs" })).toHaveAttribute(
      "href",
      "/runs?benchmark=inference.latency",
    );
  });
});
