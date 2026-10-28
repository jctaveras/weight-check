import type { ReactNode } from "react";

export interface Column<T> {
  key: string;
  header: ReactNode;
  cell: (row: T) => ReactNode;
  /** Right-align and use tabular numerals. */
  numeric?: boolean;
  className?: string;
}

interface DataTableProps<T> {
  columns: Column<T>[];
  rows: T[];
  rowKey: (row: T) => string;
  caption: string;
}

export function DataTable<T>({ columns, rows, rowKey, caption }: DataTableProps<T>) {
  return (
    <div className="-mx-5 overflow-x-auto px-5">
      <table className="w-full border-collapse text-sm">
        <caption className="sr-only">{caption}</caption>
        <thead>
          <tr className="border-b border-border text-left text-xs tracking-wide text-muted uppercase">
            {columns.map((col) => (
              <th
                key={col.key}
                scope="col"
                className={`px-3 py-2 font-medium first:pl-0 last:pr-0 ${col.numeric ? "text-right" : ""}`}
              >
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={rowKey(row)} className="border-b border-border last:border-0">
              {columns.map((col) => (
                <td
                  key={col.key}
                  className={`px-3 py-2.5 align-middle first:pl-0 last:pr-0 ${col.numeric ? "text-right tabular-nums" : ""} ${col.className ?? ""}`}
                >
                  {col.cell(row)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
