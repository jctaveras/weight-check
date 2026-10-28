// Shared Recharts styling: recessive axes and grid in the app's tokens, thin rounded bars.

export const axisTick = { fill: "var(--muted)", fontSize: 12 };
export const gridStroke = "var(--border)";
export const hoverCursor = { fill: "var(--surface-muted)" };
/** Rounded data end (right side of a horizontal bar), square at the baseline. */
export const barRadius: [number, number, number, number] = [0, 4, 4, 0];

/** Truncates long run labels on the category axis; the legend and table show them in full. */
export function truncateLabel(label: string, max = 28): string {
  return label.length > max ? `${label.slice(0, max - 1)}…` : label;
}

const signedPercent = new Intl.NumberFormat(undefined, {
  style: "percent",
  maximumFractionDigits: 1,
  signDisplay: "exceptZero",
});

/** "+12.5%" / "−3%" from a value on a 0–100 scale. */
export function formatSignedPercent(pct: number): string {
  return signedPercent.format(pct / 100);
}
