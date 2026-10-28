import { call } from "./client";
import type { ActiveModel, LoadModelRequest, ModelInfo, ModelStatus } from "./types";

export const listModels = () => call<ModelInfo[]>("list_models");

export const getModelStatus = () => call<ModelStatus>("model_status");

export const loadModel = (request: LoadModelRequest) => call<ActiveModel>("load_model", { request });

export const unloadModel = async (): Promise<void> => {
  await call<null>("unload_model");
};
