// Locale-aware display formatting. `locale` defaults to the browser's preference.

const BYTE_UNITS = ["B", "KiB", "MiB", "GiB", "TiB"] as const;

export function formatNumber(value: number, maxFractionDigits = 2, locale?: string): string {
  return new Intl.NumberFormat(locale, { maximumFractionDigits: maxFractionDigits }).format(value);
}

export function formatBytes(bytes: number | null | undefined, locale?: string): string {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes)) return "—";
  let value = bytes;
  let unit = 0;
  while (Math.abs(value) >= 1024 && unit < BYTE_UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${formatNumber(value, unit === 0 ? 0 : 1, locale)} ${BYTE_UNITS[unit]}`;
}

/** Formats a percentage given on a 0–100 scale. */
export function formatPercent(pct: number | null | undefined, locale?: string): string {
  if (pct === null || pct === undefined || !Number.isFinite(pct)) return "—";
  return new Intl.NumberFormat(locale, { style: "percent", maximumFractionDigits: 1 }).format(
    pct / 100,
  );
}

export function formatDuration(ms: number | null | undefined, locale?: string): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms)) return "—";
  if (ms < 1000) return `${formatNumber(ms, 0, locale)} ms`;
  const seconds = ms / 1000;
  if (seconds < 60) return `${formatNumber(seconds, 1, locale)} s`;
  const minutes = Math.floor(seconds / 60);
  const rest = Math.round(seconds - minutes * 60);
  return `${minutes} min ${rest} s`;
}

export function formatDateTime(iso: string | null | undefined, locale?: string): string {
  if (!iso) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "medium" }).format(
    date,
  );
}

/** Milliseconds between two ISO timestamps, or null when either is missing. */
export function elapsedMs(start: string | null, end: string | null): number | null {
  if (!start || !end) return null;
  const ms = new Date(end).getTime() - new Date(start).getTime();
  return Number.isFinite(ms) ? ms : null;
}

/** Metric values keep the backend's unit (ms, MiB, tok/s, …) so runs stay comparable. */
export function formatMetricValue(value: number, locale?: string): string {
  return formatNumber(value, Math.abs(value) >= 100 ? 1 : 2, locale);
}

/** "inference.generation_speed" → "Generation Speed"; "model_quality" → "Model Quality". */
export function humanize(id: string): string {
  const last = id.split(".").pop() ?? id;
  return last
    .split("_")
    .filter(Boolean)
    .map((w) => w[0].toUpperCase() + w.slice(1))
    .join(" ");
}

export function shortId(id: string): string {
  return id.slice(0, 8);
}
