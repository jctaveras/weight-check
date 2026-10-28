import { createFileRoute } from "@tanstack/react-router";

import { RunDetail } from "@/components/runs/RunDetail";

export const Route = createFileRoute("/runs/$id")({
  staticData: { title: "Run" },
  component: RunPage,
});

function RunPage() {
  const { id } = Route.useParams();
  // The route component stays mounted when moving between runs; key by id so local state resets.
  return <RunDetail key={id} id={id} />;
}
