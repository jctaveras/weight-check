import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";

import type { ModelInfo } from "@/lib/api/types";
import { jsonResponse, mockFetch, renderWithQuery } from "@/test/render";
import { LoadModelForm } from "./LoadModelForm";
import { parseLoadForm } from "./loadForm";

const model: ModelInfo = {
  id: "11111111-1111-1111-1111-111111111111",
  name: "tiny-q4",
  file_path: "/models/tiny-q4.gguf",
  architecture: "llama",
  quantization: "Q4_K_M",
  size_label: "1B",
  block_count: 16,
  file_size_bytes: 700_000_000,
  created_at: "2026-01-01T00:00:00Z",
};

const blank = { model_id: "", ctx_size: "", n_gpu_layers: "", parallel: "", threads: "" };

describe("parseLoadForm", () => {
  it("omits blank optional fields", () => {
    expect(parseLoadForm({ ...blank, model_id: "m" })).toEqual({ ok: true, request: { model_id: "m" } });
  });

  it("validates bounds like the backend", () => {
    const result = parseLoadForm({ model_id: "", ctx_size: "64", n_gpu_layers: "-1", parallel: "65", threads: "1.5" });
    expect(result).toEqual({
      ok: false,
      errors: {
        model_id: "Choose a model to load.",
        ctx_size: "Must be at least 128.",
        n_gpu_layers: "Must be at least 0.",
        parallel: "Must be at most 64.",
        threads: "Enter a whole number.",
      },
    });
  });
});

function Harness({ hasLoadedModel = false }: { hasLoadedModel?: boolean }) {
  const [selectedId, setSelectedId] = useState("");
  return (
    <LoadModelForm
      models={[model]}
      selectedId={selectedId}
      onSelect={setSelectedId}
      busy={false}
      hasLoadedModel={hasLoadedModel}
    />
  );
}

describe("LoadModelForm", () => {
  it("sends only the numeric fields that were filled in", async () => {
    const fetch = mockFetch(() =>
      jsonResponse({ model, settings: { ctx_size: 2048, n_gpu_layers: 8, parallel: 1, threads: null } }),
    );
    renderWithQuery(<Harness />);
    await userEvent.selectOptions(screen.getByLabelText("Model"), model.id);
    await userEvent.type(screen.getByLabelText("Context Size"), "2048");
    await userEvent.type(screen.getByLabelText("GPU Layers"), "8");
    await userEvent.click(screen.getByRole("button", { name: "Load Model" }));

    await waitFor(() => expect(fetch).toHaveBeenCalled());
    const [url, init] = fetch.mock.calls[0];
    expect(url).toMatch(/\/models\/load$/);
    expect(JSON.parse(String(init?.body))).toEqual({ model_id: model.id, ctx_size: 2048, n_gpu_layers: 8 });
    expect(await screen.findByText("Loaded tiny-q4.")).toBeInTheDocument();
  });

  it("shows inline errors and focuses the first invalid field", async () => {
    const fetch = mockFetch(() => jsonResponse({}));
    renderWithQuery(<Harness />);
    await userEvent.type(screen.getByLabelText("Parallel Slots"), "0");
    await userEvent.click(screen.getByRole("button", { name: "Load Model" }));
    expect(screen.getByText("Choose a model to load.")).toBeInTheDocument();
    expect(screen.getByText("Must be at least 1.")).toBeInTheDocument();
    expect(screen.getByLabelText("Model")).toHaveFocus();
    expect(fetch).not.toHaveBeenCalled();
  });

  it("labels the action as a swap when a model is loaded", () => {
    renderWithQuery(<Harness hasLoadedModel />);
    expect(screen.getByRole("button", { name: "Swap Model" })).toBeInTheDocument();
  });
});
