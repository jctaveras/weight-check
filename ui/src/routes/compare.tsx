import { createFileRoute } from "@tanstack/react-router";

import { CompareView } from "@/components/compare/CompareView";
import { validateCompareSearch } from "@/components/compare/compareParams";
import { PageHeader } from "@/components/ui/PageHeader";

export const Route = createFileRoute("/compare")({
  staticData: { title: "Compare" },
  validateSearch: validateCompareSearch,
  component: ComparePage,
});

function ComparePage() {
  const search = Route.useSearch();
  return (
    <>
      <PageHeader
        title="Compare"
        description="Put completed runs of one benchmark side by side. The selection is kept in the URL."
      />
      <CompareView search={search} />
    </>
  );
}
