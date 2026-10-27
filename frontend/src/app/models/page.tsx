import type { Metadata } from "next";

import { ModelsView } from "@/components/models/ModelsView";
import { PageHeader } from "@/components/ui/PageHeader";

export const metadata: Metadata = { title: "Models" };

export default function ModelsPage() {
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
