import type { LinkOptions } from "@tanstack/react-router";

import { ApiError } from "./api/client";

export interface ErrorDescription {
  message: string;
  /** What the user can do about it. */
  hint?: string;
  action?: { to: LinkOptions["to"]; label: string };
}

export function describeError(error: unknown): ErrorDescription {
  if (!(error instanceof ApiError)) {
    return { message: error instanceof Error ? error.message : "Something went wrong." };
  }
  switch (error.code) {
    case "network":
      return {
        message: error.message,
        hint: "Run `pnpm tauri dev` in ui/ to start the desktop app; a plain browser tab can't reach the core.",
      };
    case "no_model_loaded":
      return {
        message: "No model is loaded.",
        hint: "Load a model before starting a benchmark.",
        action: { to: "/models", label: "Go to Models" },
      };
    case "busy":
      return { message: error.message, hint: "Wait for the current operation to finish, then retry." };
    case "not_implemented":
      return { message: error.message, hint: "This benchmark isn't available yet." };
    case "llama_server":
      return {
        message: error.message,
        hint: "Check the app logs; try fewer GPU layers or a smaller context size.",
      };
    case "validation":
      return { message: error.message, hint: "Fix the highlighted input and submit again." };
    default:
      return { message: error.message, hint: "Check the app logs for details." };
  }
}
