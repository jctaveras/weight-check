import { apiFetch } from "./client";
import type { ActiveModel, LoadModelRequest, ModelInfo, ModelStatus } from "./types";

export const listModels = (signal?: AbortSignal) => apiFetch<ModelInfo[]>("/models", { signal });

export const getModelStatus = (signal?: AbortSignal) =>
  apiFetch<ModelStatus>("/models/status", { signal });

export const loadModel = (body: LoadModelRequest) =>
  apiFetch<ActiveModel>("/models/load", { method: "POST", body });

export const unloadModel = () => apiFetch<void>("/models/unload", { method: "POST" });
