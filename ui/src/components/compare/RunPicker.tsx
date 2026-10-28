import { Button } from "@/components/ui/Button";
import { DataTable, type Column } from "@/components/ui/DataTable";
import { EmptyState } from "@/components/ui/EmptyState";
import { ErrorMessage } from "@/components/ui/ErrorMessage";
import { LoadingText } from "@/components/ui/LoadingText";
import type { Run } from "@/lib/api/types";
import { formatDateTime } from "@/lib/format";
import { useLookups } from "@/lib/queries/useLookups";
import { useRunPages } from "@/lib/queries/useRunPages";
import { useRunsById } from "@/lib/queries/useRunsById";
import { settingsLabel } from "./comparison";
import { MAX_COMPARE_RUNS } from "./compareParams";

interface RunPickerProps {
  benchmark: string;
  selected: string[];
  onToggle: (id: string) => void;
}

/**
 * Completed runs of `benchmark`, newest first, each with a checkbox to add it to the comparison.
 * Older runs load a page at a time; picked runs that aren't on a loaded page are pinned on top so
 * they can still be deselected.
 */
export function RunPicker({ benchmark, selected, onToggle }: RunPickerProps) {
  const pages = useRunPages({ benchmark, status: "completed" });
  const listedIds = new Set(pages.data?.map((r) => r.id));
  const unlisted = pages.data ? selected.filter((id) => !listedIds.has(id)) : [];
  const pinned = useRunsById(unlisted);
  const { modelName } = useLookups();

  if (pages.isPending || pinned.some((q) => q.isPending)) {
    return <LoadingText>Loading runs…</LoadingText>;
  }
  // A failed "load older" keeps the pages already shown; only a failed first page replaces them.
  if (!pages.data) return <ErrorMessage error={pages.error} title="Couldn't load runs" />;

  const missing = unlisted.filter((_, i) => pinned[i].isError);
  const runs = [...pinned.flatMap((q) => (q.data ? [q.data] : [])), ...pages.data];
  if (runs.length === 0 && missing.length === 0) {
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
      {missing.map((id) => (
        <p key={id} className="flex items-center gap-2 text-sm text-danger">
          <span>
            Run <span className="font-mono" translate="no">{id}</span> couldn&rsquo;t be loaded. Remove it from the
            comparison.
          </span>
          <Button variant="secondary" size="sm" onClick={() => onToggle(id)}>
            Remove
          </Button>
        </p>
      ))}
      <div className="max-h-80 overflow-y-auto">
        <DataTable caption="Completed runs to compare" columns={columns} rows={runs} rowKey={(r) => r.id} />
      </div>
      {pages.hasNextPage && (
        <div>
          <Button
            variant="secondary"
            size="sm"
            loading={pages.isFetchingNextPage}
            onClick={() => void pages.fetchNextPage()}
          >
            {pages.isFetchingNextPage ? "Loading Older Runs…" : "Load Older Runs"}
          </Button>
        </div>
      )}
      {pages.isFetchNextPageError && (
        <ErrorMessage error={pages.error} title="Couldn't load older runs" />
      )}
    </div>
  );
}
