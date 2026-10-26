"use client";

import Link from "next/link";
import { useSearchParams } from "next/navigation";

import { Card } from "@/components/ui/Card";
import { EmptyState } from "@/components/ui/EmptyState";
import { ErrorMessage } from "@/components/ui/ErrorMessage";
import { LoadingText } from "@/components/ui/LoadingText";
import { useRuns } from "@/lib/queries/useRuns";
import { RunFilters } from "./RunFilters";
import { RunsTable } from "./RunsTable";
import { filterFromSearchParams } from "./runFilterParams";

export function RunsView() {
  const searchParams = useSearchParams();
  const filter = filterFromSearchParams(new URLSearchParams(searchParams.toString()));
  const { data, error, isPending } = useRuns(filter);

  return (
    <div className="flex flex-col gap-6">
      <RunFilters filter={filter} />
      <Card>
        {isPending ? (
          <LoadingText>Loading runs…</LoadingText>
        ) : error ? (
          <ErrorMessage error={error} title="Couldn't load runs" />
        ) : data.length === 0 ? (
          <EmptyState title="No Runs Yet">
            <Link href="/benchmarks" className="font-medium text-accent underline underline-offset-2">
              Start a benchmark
            </Link>{" "}
            and its results will show up here.
          </EmptyState>
        ) : (
          <RunsTable runs={data} />
        )}
      </Card>
    </div>
  );
}
