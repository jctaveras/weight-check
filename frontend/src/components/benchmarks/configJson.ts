import type { JsonValue } from "@/lib/api/types";

export type ConfigParseResult =
  | { ok: true; value: { [key: string]: JsonValue } }
  | { ok: false; error: string };

export function stringifyConfig(config: JsonValue): string {
  return JSON.stringify(config ?? {}, null, 2);
}

/** Parses the config textarea; the backend expects a JSON object (unknown fields are rejected there). */
export function parseConfig(text: string): ConfigParseResult {
  const trimmed = text.trim();
  if (trimmed === "") return { ok: true, value: {} };
  let value: unknown;
  try {
    value = JSON.parse(trimmed);
  } catch (err) {
    const detail = err instanceof SyntaxError ? err.message : "invalid JSON";
    return { ok: false, error: `Invalid JSON: ${detail}` };
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return { ok: false, error: "Config must be a JSON object, e.g. { \"repetitions\": 1 }." };
  }
  return { ok: true, value: value as { [key: string]: JsonValue } };
}
