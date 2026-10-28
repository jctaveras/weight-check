import { DataTable, type Column } from "@/components/ui/DataTable";
import { EmptyState } from "@/components/ui/EmptyState";
import type { Metric } from "@/lib/api/types";
import { formatMetricValue } from "@/lib/format";

const columns: Column<Metric>[] = [
  {
    key: "name",
    header: "Metric",
    className: "w-full",
    cell: (m) => (
      <span className="font-mono text-xs" translate="no">
        {m.name}
      </span>
    ),
  },
  { key: "value", header: "Value", numeric: true, cell: (m) => formatMetricValue(m.value) },
  {
    key: "unit",
    header: "Unit",
    cell: (m) => (
      <span className="whitespace-nowrap text-muted" translate="no">
        {m.unit || "—"}
      </span>
    ),
  },
];

export function MetricsTable({ metrics }: { metrics: Metric[] }) {
  if (metrics.length === 0) {
    return <EmptyState title="No Metrics Recorded">This run didn&rsquo;t report any metrics.</EmptyState>;
  }
  return <DataTable caption="Run metrics" columns={columns} rows={metrics} rowKey={(m) => m.name} />;
}
