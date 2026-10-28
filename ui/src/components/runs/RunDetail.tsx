import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { Card } from "@/components/ui/Card";
import { ErrorMessage } from "@/components/ui/ErrorMessage";
import { LoadingText } from "@/components/ui/LoadingText";
import { PageHeader } from "@/components/ui/PageHeader";
import { elapsedMs, formatDateTime, formatDuration } from "@/lib/format";
import { useLookups } from "@/lib/queries/useLookups";
import { useRun } from "@/lib/queries/useRun";
import { isActive } from "@/lib/queries/useRuns";
import { JsonViewer } from "./JsonViewer";
import { MetricsTable } from "./MetricsTable";
import { RunStatusBadge } from "./RunStatusBadge";

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="min-w-0">
      <dt className="text-xs tracking-wide text-muted uppercase">{label}</dt>
      <dd className="truncate text-sm font-medium tabular-nums">{children}</dd>
    </div>
  );
}

export function RunDetail({ id }: { id: string }) {
  const { data: run, error, isPending } = useRun(id);
  const { benchmarkName, categoryTitle, modelName } = useLookups();

  if (isPending) return <LoadingText>Loading run…</LoadingText>;
  if (error && !run) return <ErrorMessage error={error} title="Couldn't load this run" />;

  const active = isActive(run.status);

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title={benchmarkName(run.benchmark_name)}
        description={
          <>
            {categoryTitle(run.category)} ·{" "}
            <span className="font-mono" translate="no">
              {run.id}
            </span>
          </>
        }
        actions={
          <>
            <Link
              to="/runs"
              search={{ benchmark: run.benchmark_name }}
              className="text-sm text-muted underline-offset-2 hover:text-fg hover:underline"
            >
              Past runs
            </Link>
            {run.status === "completed" && (
              <Link
                to="/compare"
                search={{ benchmark: run.benchmark_name, runs: run.id }}
                className="text-sm text-muted underline-offset-2 hover:text-fg hover:underline"
              >
                Compare With Other Runs
              </Link>
            )}
          </>
        }
      />

      <Card
        title="Status"
        actions={
          <span aria-live="polite">
            <RunStatusBadge status={run.status} />
          </span>
        }
      >
        <dl className="grid grid-cols-2 gap-4 sm:grid-cols-4">
          <Field label="Model">
            <span title={modelName(run.model_id)} translate="no">
              {modelName(run.model_id)}
            </span>
          </Field>
          <Field label="Created">{formatDateTime(run.created_at)}</Field>
          <Field label="Started">{formatDateTime(run.started_at)}</Field>
          <Field label="Duration">{formatDuration(elapsedMs(run.started_at, run.finished_at))}</Field>
        </dl>
        {active && (
          <div className="mt-4">
            <LoadingText>
              {run.status === "queued" ? "Waiting to start…" : "Benchmark running; results appear when it finishes…"}
            </LoadingText>
          </div>
        )}
        {run.error && (
          <div className="mt-4">
            <ErrorMessage error={new Error(run.error)} title="Run failed" />
          </div>
        )}
      </Card>

      <Card title="Metrics">
        {active ? <LoadingText>Collecting metrics…</LoadingText> : <MetricsTable metrics={run.metrics} />}
      </Card>

      <div className="flex flex-col gap-3">
        <JsonViewer label="Config" value={run.config} />
        <JsonViewer label="Details" value={run.details} />
      </div>
    </div>
  );
}
