import { Link } from "@tanstack/react-router";

import { buttonClasses } from "@/components/ui/buttonClasses";
import { Card } from "@/components/ui/Card";
import { EmptyState } from "@/components/ui/EmptyState";
import { ErrorMessage } from "@/components/ui/ErrorMessage";
import { LoadingText } from "@/components/ui/LoadingText";
import { useRuns } from "@/lib/queries/useRuns";
import { RunsTable } from "./RunsTable";

export function RecentRuns({ limit = 5 }: { limit?: number }) {
  const { data, error, isPending } = useRuns({ limit });

  return (
    <Card
      title="Recent Runs"
      actions={
        <Link to="/runs" className={buttonClasses("ghost", "sm")}>
          View All
        </Link>
      }
    >
      {isPending ? (
        <LoadingText>Loading runs…</LoadingText>
      ) : error ? (
        <ErrorMessage error={error} title="Couldn't load runs" />
      ) : data.length === 0 ? (
        <EmptyState title="No Runs Yet">
          <Link to="/benchmarks" className="font-medium text-accent underline underline-offset-2">
            Start a benchmark
          </Link>{" "}
          to see results here.
        </EmptyState>
      ) : (
        <RunsTable runs={data} caption="Recent benchmark runs" />
      )}
    </Card>
  );
}
