import { DataTable, type Column } from "@/components/ui/DataTable";
import { formatMetricValue } from "@/lib/format";
import { formatSignedPercent } from "./chartTheme";
import { deltaFromBest, type MetricRow, type Series } from "./comparison";

const DIRECTION_HINT = { higher: "↑ higher is better", lower: "↓ lower is better", neutral: null };

function ValueCell({ row, index }: { row: MetricRow; index: number }) {
  const value = row.values[index];
  if (value === undefined) return <span className="text-muted">—</span>;
  const best = row.best === undefined ? undefined : row.values[row.best];
  const delta = best === undefined || row.best === index ? undefined : deltaFromBest(value, best);
  return (
    <div className="flex flex-col items-end">
      <span className={row.best === index ? "font-semibold" : ""}>{formatMetricValue(value)}</span>
      {row.best === index ? (
        <span className="text-xs font-medium text-success">Best</span>
      ) : (
        delta !== undefined && <span className="text-xs text-muted">{formatSignedPercent(delta)}</span>
      )}
    </div>
  );
}

/** Every metric of the compared runs, with the best value marked and the others' gap to it. */
export function ComparisonTable({ series, metrics }: { series: Series[]; metrics: MetricRow[] }) {
  const columns: Column<MetricRow>[] = [
    {
      key: "metric",
      header: "Metric",
      cell: (m) => (
        <div className="flex flex-col">
          <span className="font-mono text-xs whitespace-nowrap" translate="no">
            {m.name} <span className="text-muted">({m.unit || "—"})</span>
          </span>
          {DIRECTION_HINT[m.direction] && <span className="text-xs text-muted">{DIRECTION_HINT[m.direction]}</span>}
        </div>
      ),
    },
    ...series.map((s, i) => ({
      key: s.runId,
      numeric: true,
      header: (
        <span className="inline-flex max-w-40 items-center gap-1.5 normal-case" title={s.label}>
          <span aria-hidden className="size-2.5 shrink-0 rounded-sm" style={{ background: s.color }} />
          <span className="truncate" translate="no">
            {s.label}
          </span>
        </span>
      ),
      cell: (m: MetricRow) => <ValueCell row={m} index={i} />,
    })),
  ];
  return <DataTable caption="Metrics of the compared runs" columns={columns} rows={metrics} rowKey={(m) => m.name} />;
}
