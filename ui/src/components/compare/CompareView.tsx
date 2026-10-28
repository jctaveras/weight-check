import { useNavigate } from "@tanstack/react-router";
import { useMemo } from "react";

import { Card } from "@/components/ui/Card";
import { EmptyState } from "@/components/ui/EmptyState";
import { ErrorMessage } from "@/components/ui/ErrorMessage";
import { LoadingText } from "@/components/ui/LoadingText";
import { SelectField } from "@/components/ui/SelectField";
import { useCompareRuns } from "@/lib/queries/useCompareRuns";
import { useLookups } from "@/lib/queries/useLookups";
import { ComparisonTable } from "./ComparisonTable";
import { buildComparison, headlineMetrics } from "./comparison";
import { runIds, toggleRun, withBenchmark, type CompareSearch } from "./compareParams";
import { MetricChart } from "./MetricChart";
import { OverviewChart } from "./OverviewChart";
import { RunPicker } from "./RunPicker";
import { SeriesLegend } from "./SeriesLegend";

function Results({ ids }: { ids: string[] }) {
  const { data, error, isPending } = useCompareRuns(ids);
  const { modelName } = useLookups();
  const comparison = useMemo(() => data && buildComparison(data, modelName), [data, modelName]);

  if (isPending) return <LoadingText>Loading comparison…</LoadingText>;
  if (error || !comparison) return <ErrorMessage error={error} title="Couldn't compare these runs" />;

  const { series, metrics } = comparison;
  const headline = headlineMetrics(metrics);
  return (
    <>
      {headline.length > 0 && (
        <Card title="Overview">
          <div className="flex flex-col gap-4">
            <p className="text-sm text-muted">
              Each bar is a run&rsquo;s score relative to the best run for that metric (100%). Longer is
              better, whether the metric is higher- or lower-is-better.
            </p>
            <SeriesLegend series={series} />
            <OverviewChart series={series} metrics={headline} />
          </div>
        </Card>
      )}
      <Card title="Metric Detail">
        <MetricChart series={series} metrics={metrics} initial={headline[0]?.name} />
      </Card>
      <Card title="All Metrics">
        <ComparisonTable series={series} metrics={metrics} />
      </Card>
    </>
  );
}

export function CompareView({ search }: { search: CompareSearch }) {
  const navigate = useNavigate({ from: "/compare" });
  const { catalog } = useLookups();
  const ids = runIds(search);

  const update = (next: (prev: CompareSearch) => CompareSearch) =>
    navigate({ search: next, replace: true, resetScroll: false });

  return (
    <div className="flex flex-col gap-6">
      <Card title="Runs">
        <div className="flex flex-col gap-4">
          <div className="sm:w-80">
            <SelectField
              label="Benchmark"
              value={search.benchmark ?? ""}
              onChange={(benchmark) => update((prev) => withBenchmark(prev, benchmark))}
            >
              <option value="">Choose a benchmark…</option>
              {catalog.map((c) => {
                const benchmarks = c.benchmarks.filter((b) => b.implemented);
                return (
                  benchmarks.length > 0 && (
                    <optgroup key={c.category} label={c.title}>
                      {benchmarks.map((b) => (
                        <option key={b.id} value={b.id}>
                          {b.name}
                        </option>
                      ))}
                    </optgroup>
                  )
                );
              })}
            </SelectField>
          </div>
          {search.benchmark && (
            <RunPicker
              benchmark={search.benchmark}
              selected={ids}
              onToggle={(id) => update((prev) => toggleRun(prev, id))}
            />
          )}
        </div>
      </Card>

      {!search.benchmark ? (
        <EmptyState title="Choose a Benchmark">Pick a benchmark above to list its completed runs.</EmptyState>
      ) : ids.length < 2 ? (
        <EmptyState title="Pick at Least Two Runs">
          Select two or more runs above to chart them side by side.
        </EmptyState>
      ) : (
        <Results ids={ids} />
      )}
    </div>
  );
}
