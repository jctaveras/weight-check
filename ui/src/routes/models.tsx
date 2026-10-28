import { createFileRoute } from "@tanstack/react-router";

import { ModelsView } from "@/components/models/ModelsView";
import { PageHeader } from "@/components/ui/PageHeader";

export const Route = createFileRoute("/models")({
  staticData: { title: "Models" },
  component: ModelsPage,
});

function ModelsPage() {
  return (
    <>
      <PageHeader
        title="Models"
        description="GGUF files found in MODELS_DIR. Only one model can be loaded at a time."
      />
      <ModelsView />
    </>
  );
}
