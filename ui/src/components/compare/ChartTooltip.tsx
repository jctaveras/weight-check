import type { ReactNode } from "react";

interface ChartTooltipProps {
  title: ReactNode;
  rows: { key: string; color: string; label: string; value: ReactNode }[];
}

/** Tooltip body for the compare charts; values in text ink, color only on the swatch. */
export function ChartTooltip({ title, rows }: ChartTooltipProps) {
  return (
    <div className="rounded-md border border-border bg-surface px-3 py-2 text-xs shadow-md">
      <p className="mb-1.5 font-mono font-medium" translate="no">
        {title}
      </p>
      <ul className="flex flex-col gap-1">
        {rows.map((row) => (
          <li key={row.key} className="flex items-center gap-2">
            <span aria-hidden className="size-2.5 shrink-0 rounded-sm" style={{ background: row.color }} />
            <span className="max-w-56 truncate text-muted" translate="no">
              {row.label}
            </span>
            <span className="ml-auto pl-3 font-medium tabular-nums">{row.value}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
