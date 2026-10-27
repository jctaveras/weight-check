import { apiFetch } from "./client";
import type { SystemSnapshot } from "./types";

export const getSystem = (signal?: AbortSignal) =>
  apiFetch<SystemSnapshot>("/system", { signal });
