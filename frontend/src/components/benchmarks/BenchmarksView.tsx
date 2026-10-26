"use client";

import Link from "next/link";

import { ErrorMessage } from "@/components/ui/ErrorMessage";
import { LoadingText } from "@/components/ui/LoadingText";
import { useCatalog } from "@/lib/queries/useCatalog";
import { useModelStatus } from "@/lib/queries/useModelStatus";
import { CategorySection } from "./CategorySection";

function ModelNotice() {
  const { data: status } = useModelStatus();
  if (!status) return null;
  if (status.loaded) {
    return (
      <p className="rounded-md border border-border bg-surface px-4 py-3 text-sm" aria-live="polite">
        Runs will use{" "}
        <span className="font-medium" translate="no">
          {status.loaded.model.name}
        </span>
        .
      </p>
    );
  }
  return (
    <p className="rounded-md border border-accent/40 bg-accent-soft px-4 py-3 text-sm" aria-live="polite">
      No model is loaded.{" "}
      <Link href="/models" className="font-medium text-accent underline underline-offset-2">
        Load a model
      </Link>{" "}
      before starting a run.
    </p>
  );
}

export function BenchmarksView() {
  const { data, error, isPending } = useCatalog();

  if (isPending) return <LoadingText>Loading benchmarks…</LoadingText>;
  if (error) return <ErrorMessage error={error} title="Couldn't load the benchmark catalog" />;

  return (
    <div className="flex flex-col gap-10">
      <ModelNotice />
      {data.map((category) => (
        <CategorySection key={category.category} category={category} />
      ))}
    </div>
  );
}
