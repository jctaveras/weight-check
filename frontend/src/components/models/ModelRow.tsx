import { Badge } from "@/components/ui/Badge";
import type { ModelInfo } from "@/lib/api/types";

interface ModelNameCellProps {
  model: ModelInfo;
  loaded: boolean;
  loading: boolean;
}

/** Name + file path cell for the models table, with load state badges. */
export function ModelNameCell({ model, loaded, loading }: ModelNameCellProps) {
  return (
    <div className="flex min-w-0 max-w-md flex-col gap-0.5">
      <div className="flex min-w-0 items-center gap-2">
        <span className="truncate font-medium" title={model.name} translate="no">
          {model.name}
        </span>
        {loaded && <Badge tone="success">Loaded</Badge>}
        {loading && <Badge tone="accent">Loading…</Badge>}
      </div>
      <span className="truncate font-mono text-xs text-muted" title={model.file_path} translate="no">
        {model.file_path}
      </span>
    </div>
  );
}
