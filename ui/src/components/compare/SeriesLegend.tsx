import type { Series } from "./comparison";

/** Run identity for the charts: a swatch plus the full label, so color is never the only cue. */
export function SeriesLegend({ series }: { series: Series[] }) {
  return (
    <ul className="flex flex-wrap gap-x-4 gap-y-1.5 text-sm" aria-label="Runs">
      {series.map((s) => (
        <li key={s.runId} className="flex min-w-0 items-center gap-2">
          <span aria-hidden className="size-3 shrink-0 rounded-sm" style={{ background: s.color }} />
          <span className="truncate" translate="no">
            {s.label}
          </span>
        </li>
      ))}
    </ul>
  );
}
