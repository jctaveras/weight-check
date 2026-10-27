import { createFileRoute } from "@tanstack/react-router";

import { BenchmarksView } from "@/components/benchmarks/BenchmarksView";
import { PageHeader } from "@/components/ui/PageHeader";

export const Route = createFileRoute("/benchmarks")({
  staticData: { title: "Benchmarks" },
  component: BenchmarksPage,
});

function BenchmarksPage() {
  return (
    <>
      <PageHeader
        title="Benchmarks"
        description="Runs execute against the loaded model with deterministic settings (temperature 0, no prompt cache)."
      />
      <BenchmarksView />
    </>
  );
}
