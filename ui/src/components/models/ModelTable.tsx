import { Button } from "@/components/ui/Button";
import { DataTable, type Column } from "@/components/ui/DataTable";
import { EmptyState } from "@/components/ui/EmptyState";
import type { ModelInfo } from "@/lib/api/types";
import { formatBytes } from "@/lib/format";
import { ModelNameCell } from "./ModelRow";

interface ModelTableProps {
  models: ModelInfo[];
  loadedId: string | null;
  loadingId: string | null;
  selectedId: string;
  onSelect: (id: string) => void;
}

export function ModelTable({ models, loadedId, loadingId, selectedId, onSelect }: ModelTableProps) {
  if (models.length === 0) {
    return (
      <EmptyState title="No Models Found">
        Put <code className="font-mono">*.gguf</code> files in <code className="font-mono">MODELS_DIR</code>{" "}
        and refresh.
      </EmptyState>
    );
  }

  const columns: Column<ModelInfo>[] = [
    {
      key: "name",
      header: "Model",
      className: "max-w-0 w-full",
      cell: (m) => (
        <ModelNameCell model={m} loaded={m.id === loadedId} loading={m.id === loadingId} />
      ),
    },
    { key: "arch", header: "Arch", cell: (m) => <span translate="no">{m.architecture ?? "—"}</span> },
    { key: "quant", header: "Quant", cell: (m) => <span translate="no">{m.quantization ?? "—"}</span> },
    { key: "params", header: "Params", cell: (m) => m.size_label ?? "—" },
    { key: "layers", header: "Layers", numeric: true, cell: (m) => m.block_count ?? "—" },
    { key: "size", header: "Size", numeric: true, cell: (m) => formatBytes(m.file_size_bytes) },
    {
      key: "action",
      header: <span className="sr-only">Actions</span>,
      cell: (m) => (
        <Button
          size="sm"
          variant={m.id === selectedId ? "primary" : "secondary"}
          aria-pressed={m.id === selectedId}
          aria-label={`Select ${m.name}`}
          onClick={() => onSelect(m.id)}
        >
          {m.id === selectedId ? "Selected" : "Select"}
        </Button>
      ),
    },
  ];

  return <DataTable caption="Available models" columns={columns} rows={models} rowKey={(m) => m.id} />;
}
