import { useMemo } from "react";

import { humanize } from "@/lib/format";
import { useCatalog } from "./useCatalog";
import { useModels } from "./useModels";

/** Display-name lookups for ids that runs reference. */
export function useLookups() {
  const { data: catalog } = useCatalog();
  const { data: models } = useModels();

  return useMemo(() => {
    const benchmarks = new Map(
      (catalog ?? []).flatMap((c) => c.benchmarks.map((b) => [b.id, b.name] as const)),
    );
    const categories = new Map((catalog ?? []).map((c) => [c.category, c.title] as const));
    const modelNames = new Map((models ?? []).map((m) => [m.id, m.name] as const));
    return {
      benchmarkName: (id: string) => benchmarks.get(id) ?? humanize(id),
      categoryTitle: (id: string) => categories.get(id as never) ?? humanize(id),
      modelName: (id: string) => modelNames.get(id) ?? id.slice(0, 8),
      catalog: catalog ?? [],
      models: models ?? [],
    };
  }, [catalog, models]);
}
