import { Link } from "@tanstack/react-router";

import { DataTable, type Column } from "@/components/ui/DataTable";
import type { Run } from "@/lib/api/types";
import { elapsedMs, formatDateTime, formatDuration, shortId } from "@/lib/format";
import { useLookups } from "@/lib/queries/useLookups";
import { isActive } from "@/lib/queries/useRuns";
import { DeleteRunButton } from "./DeleteRunButton";
import { RunStatusBadge } from "./RunStatusBadge";

interface RunsTableProps {
  runs: Run[];
  caption?: string;
  /** Adds a Delete Run action to finished rows. */
  deletable?: boolean;
}

export function RunsTable({ runs, caption = "Benchmark runs", deletable = false }: RunsTableProps) {
  const { benchmarkName, modelName } = useLookups();

  const columns: Column<Run>[] = [
    {
      key: "benchmark",
      header: "Benchmark",
      className: "max-w-0 w-full",
      cell: (r) => (
        <div className="flex min-w-0 flex-col">
          <Link
            to="/runs/$id"
            params={{ id: r.id }}
            className="truncate font-medium text-fg underline-offset-2 hover:text-accent hover:underline"
          >
            {benchmarkName(r.benchmark_name)}
          </Link>
          <span className="font-mono text-xs text-muted" translate="no">
            {shortId(r.id)}
          </span>
        </div>
      ),
    },
    {
      key: "model",
      header: "Model",
      cell: (r) => (
        <span className="block max-w-48 truncate" title={modelName(r.model_id)} translate="no">
          {modelName(r.model_id)}
        </span>
      ),
    },
    { key: "status", header: "Status", cell: (r) => <RunStatusBadge status={r.status} /> },
    {
      key: "created",
      header: "Created",
      cell: (r) => <span className="whitespace-nowrap tabular-nums">{formatDateTime(r.created_at)}</span>,
    },
    {
      key: "duration",
      header: "Duration",
      numeric: true,
      cell: (r) => formatDuration(elapsedMs(r.started_at, r.finished_at)),
    },
  ];
  if (deletable) {
    columns.push({
      key: "actions",
      header: <span className="sr-only">Actions</span>,
      // Active runs are cancelled from their detail page.
      cell: (r) => (isActive(r.status) ? null : <DeleteRunButton runId={r.id} />),
    });
  }

  return <DataTable caption={caption} columns={columns} rows={runs} rowKey={(r) => r.id} />;
}
