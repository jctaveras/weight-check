import { createFileRoute } from "@tanstack/react-router";

import { RunsView } from "@/components/runs/RunsView";
import { filterFromSearch, validateRunSearch } from "@/components/runs/runFilterParams";
import { PageHeader } from "@/components/ui/PageHeader";

export const Route = createFileRoute("/runs/")({
  staticData: { title: "Runs" },
  validateSearch: validateRunSearch,
  component: RunsPage,
});

function RunsPage() {
  const search = Route.useSearch();
  return (
    <>
      <PageHeader title="Runs" description="Every benchmark run, newest first. Filters are kept in the URL." />
      <RunsView filter={filterFromSearch(search)} />
    </>
  );
}
