import type { ErrorCode, ErrorEnvelope } from "./types";

export const API_URL = (process.env.NEXT_PUBLIC_API_URL ?? "http://127.0.0.1:8787").replace(
  /\/+$/,
  "",
);

/** Error raised for any non-2xx response, carrying the backend's `{error:{code,message}}`. */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: ErrorCode | "network" | "unknown",
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

function isErrorEnvelope(body: unknown): body is ErrorEnvelope {
  if (typeof body !== "object" || body === null || !("error" in body)) return false;
  const error = (body as { error: unknown }).error;
  return typeof error === "object" && error !== null && "code" in error && "message" in error;
}

type QueryValue = string | number | undefined;

export interface RequestOptions {
  method?: "GET" | "POST";
  body?: unknown;
  query?: Record<string, QueryValue>;
  signal?: AbortSignal;
}

export function buildUrl(path: string, query?: Record<string, QueryValue>): string {
  const url = new URL(`${API_URL}${path}`);
  for (const [key, value] of Object.entries(query ?? {})) {
    if (value !== undefined && value !== "") url.searchParams.set(key, String(value));
  }
  return url.toString();
}

/** Fetches a backend endpoint and returns its JSON body (`undefined` for 204). */
export async function apiFetch<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const { method = "GET", body, query, signal } = options;
  let response: Response;
  try {
    response = await fetch(buildUrl(path, query), {
      method,
      signal,
      headers: body === undefined ? undefined : { "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch (err) {
    if (err instanceof DOMException && err.name === "AbortError") throw err;
    throw new ApiError(0, "network", `Can't reach the backend at ${API_URL}`);
  }

  if (response.status === 204) return undefined as T;

  const text = await response.text();
  let parsed: unknown = undefined;
  try {
    parsed = text ? JSON.parse(text) : undefined;
  } catch {
    // Non-JSON body; handled below.
  }

  if (!response.ok) {
    if (isErrorEnvelope(parsed)) {
      throw new ApiError(response.status, parsed.error.code, parsed.error.message);
    }
    throw new ApiError(
      response.status,
      "unknown",
      text || `Request failed with status ${response.status}`,
    );
  }
  return parsed as T;
}
