import type { LoadModelRequest } from "@/lib/api/types";

export type LoadFormField = "model_id" | "ctx_size" | "n_gpu_layers" | "parallel" | "threads";

export type LoadFormResult =
  | { ok: true; request: LoadModelRequest }
  | { ok: false; errors: Partial<Record<LoadFormField, string>> };

// Bounds mirror validate_settings() in backend/src/service/model_service.rs.
const RULES: Record<Exclude<LoadFormField, "model_id">, { min?: number; max?: number }> = {
  ctx_size: { min: 128 },
  n_gpu_layers: { min: 0 },
  parallel: { min: 1, max: 64 },
  threads: {},
};

/** Turns the load form's raw values into a request; blank optional fields use backend defaults. */
export function parseLoadForm(values: Record<LoadFormField, string>): LoadFormResult {
  const errors: Partial<Record<LoadFormField, string>> = {};
  const request: LoadModelRequest = { model_id: values.model_id.trim() };
  if (!request.model_id) errors.model_id = "Choose a model to load.";

  for (const [field, { min, max }] of Object.entries(RULES) as [
    Exclude<LoadFormField, "model_id">,
    { min?: number; max?: number },
  ][]) {
    const raw = values[field].trim();
    if (raw === "") continue;
    const value = Number(raw);
    if (!Number.isInteger(value)) {
      errors[field] = "Enter a whole number.";
    } else if (min !== undefined && value < min) {
      errors[field] = `Must be at least ${min}.`;
    } else if (max !== undefined && value > max) {
      errors[field] = `Must be at most ${max}.`;
    } else {
      request[field] = value;
    }
  }

  return Object.keys(errors).length ? { ok: false, errors } : { ok: true, request };
}
