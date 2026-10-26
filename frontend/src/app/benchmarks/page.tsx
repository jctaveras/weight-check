import type { Metadata } from "next";

import { BenchmarksView } from "@/components/benchmarks/BenchmarksView";
import { PageHeader } from "@/components/ui/PageHeader";

export const metadata: Metadata = { title: "Benchmarks" };

export default function BenchmarksPage() {
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
