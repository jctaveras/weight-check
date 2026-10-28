import { DataTable, type Column } from "@/components/ui/DataTable";
import { EmptyState } from "@/components/ui/EmptyState";
import { ErrorMessage } from "@/components/ui/ErrorMessage";
import { LoadingText } from "@/components/ui/LoadingText";
import type { Run } from "@/lib/api/types";
import { formatDateTime } from "@/lib/format";
import { useLookups } from "@/lib/queries/useLookups";
import { useRuns } from "@/lib/queries/useRuns";
import { settingsLabel } from "./comparison";
import { MAX_COMPARE_RUNS } from "./compareParams";

interface RunPickerProps {
  benchmark: string;
  selected: string[];
  onToggle: (id: string) => void;
}

/** Completed runs of `benchmark`, newest first, each with a checkbox to add it to the comparison. */
export function RunPicker({ benchmark, selected, onToggle }: RunPickerProps) {
  const { data, error, isPending } = useRuns({ benchmark, limit: 200 });
  const { modelName } = useLookups();

  if (isPending) return <LoadingText>Loading runs…</LoadingText>;
  if (error) return <ErrorMessage error={error} title="Couldn't load runs" />;

  const runs = data.filter((r) => r.status === "completed");
  if (runs.length === 0) {
    return (
      <EmptyState title="No Completed Runs">
        Run this benchmark on a model first; finished runs show up here.
      </EmptyState>
    );
  }

  const full = selected.length >= MAX_COMPARE_RUNS;
  const columns: Column<Run>[] = [
    {
      key: "pick",
      header: <span className="sr-only">Compare</span>,
      cell: (r) => {
        const checked = selected.includes(r.id);
        return (
          <input
            type="checkbox"
            className="size-4 accent-accent"
            checked={checked}
            disabled={!checked && full}
            onChange={() => onToggle(r.id)}
            aria-label={`Compare ${modelName(r.model_id)} run from ${formatDateTime(r.created_at)}`}
          />
        );
      },
    },
    {
      key: "model",
      header: "Model",
      className: "max-w-0 w-full",
      cell: (r) => (
        <span className="block truncate font-medium" title={modelName(r.model_id)} translate="no">
          {modelName(r.model_id)}
        </span>
      ),
    },
    {
      key: "settings",
      header: "Settings",
      cell: (r) => (
        <span className="whitespace-nowrap text-muted tabular-nums">
          {settingsLabel(r) || "—"}
        </span>
      ),
    },
    {
      key: "created",
      header: "Created",
      cell: (r) => <span className="whitespace-nowrap tabular-nums">{formatDateTime(r.created_at)}</span>,
    },
  ];

  return (
    <div className="flex flex-col gap-2">
      <p className="text-xs text-muted" aria-live="polite">
        {selected.length} of {MAX_COMPARE_RUNS} runs selected
        {full && "; deselect one to add another"}.
      </p>
      <div className="max-h-80 overflow-y-auto">
        <DataTable caption="Completed runs to compare" columns={columns} rows={runs} rowKey={(r) => r.id} />
      </div>
    </div>
  );
}
