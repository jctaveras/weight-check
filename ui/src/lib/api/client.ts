import { invoke, isTauri } from "@tauri-apps/api/core";

import type { ErrorCode } from "./types";

export type ApiErrorCode = ErrorCode | "network" | "unknown";

/** Error raised when a Tauri command rejects, carrying the core's `{ code, message }`. */
export class ApiError extends Error {
  constructor(
    readonly code: ApiErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

const ERROR_CODES: readonly ErrorCode[] = [
  "not_found",
  "no_model_loaded",
  "busy",
  "validation",
  "not_implemented",
  "llama_server",
  "internal",
];

function isErrorCode(value: unknown): value is ErrorCode {
  return typeof value === "string" && (ERROR_CODES as readonly string[]).includes(value);
}

/** Maps whatever a command rejected with to an `ApiError`. */
export function toApiError(error: unknown): ApiError {
  if (error instanceof ApiError) return error;
  if (typeof error === "object" && error !== null && "code" in error && "message" in error) {
    const { code, message } = error as { code: unknown; message: unknown };
    return new ApiError(isErrorCode(code) ? code : "unknown", String(message));
  }
  // Tauri rejects with a plain string when it can't deserialize a command's arguments.
  if (typeof error === "string") return new ApiError("validation", error);
  if (error instanceof Error) return new ApiError("unknown", error.message);
  return new ApiError("unknown", "The desktop core returned an unexpected error.");
}

/**
 * Invokes a Tauri command and resolves with its JSON result. Command names are snake_case;
 * argument keys are camelCase (Tauri maps them to the Rust parameters).
 */
export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw new ApiError(
      "network",
      "The desktop core isn't available. Open weight-check through the desktop app (pnpm tauri dev in ui/).",
    );
  }
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toApiError(error);
  }
}
