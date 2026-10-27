import type { Metadata } from "next";
import { Suspense } from "react";

import { RunsView } from "@/components/runs/RunsView";
import { LoadingText } from "@/components/ui/LoadingText";
import { PageHeader } from "@/components/ui/PageHeader";

export const metadata: Metadata = { title: "Runs" };

export default function RunsPage() {
  return (
    <>
      <PageHeader title="Runs" description="Every benchmark run, newest first. Filters are kept in the URL." />
      {/* RunsView reads useSearchParams, which needs a Suspense boundary when prerendering. */}
      <Suspense fallback={<LoadingText>Loading runs…</LoadingText>}>
        <RunsView />
      </Suspense>
    </>
  );
}
