import type { JsonValue } from "@/lib/api/types";

interface JsonViewerProps {
  label: string;
  value: JsonValue | null;
  defaultOpen?: boolean;
}

/** Collapsible, pretty-printed JSON block. */
export function JsonViewer({ label, value, defaultOpen = false }: JsonViewerProps) {
  return (
    <details open={defaultOpen} className="rounded-md border border-border">
      <summary className="cursor-pointer px-4 py-2.5 text-sm font-medium select-none hover:bg-surface-muted">
        {label}
      </summary>
      <pre
        className="max-h-[32rem] overflow-auto border-t border-border bg-surface-muted p-4 font-mono text-xs leading-relaxed"
        translate="no"
      >
        {value === null ? "null" : JSON.stringify(value, null, 2)}
      </pre>
    </details>
  );
}
