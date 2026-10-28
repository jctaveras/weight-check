import { Link, useNavigate } from "@tanstack/react-router";
import { useId, type ReactNode } from "react";

import { inputClasses } from "@/components/ui/Field";
import type { RunFilter } from "@/lib/api/types";
import { useLookups } from "@/lib/queries/useLookups";
import { searchWith } from "./runFilterParams";

function SelectFilter({
  label,
  value,
  onChange,
  children,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <label htmlFor={id} className="text-xs font-medium text-muted">
        {label}
      </label>
      <select id={id} value={value} onChange={(e) => onChange(e.target.value)} className={inputClasses}>
        {children}
      </select>
    </div>
  );
}

export function RunFilters({ filter }: { filter: RunFilter }) {
  const navigate = useNavigate({ from: "/runs/" });
  const { catalog, models } = useLookups();

  const set = (key: keyof RunFilter) => (value: string) => {
    navigate({ search: (prev) => searchWith(prev, key, value), replace: true, resetScroll: false });
  };

  const benchmarks = catalog
    .filter((c) => !filter.category || c.category === filter.category)
    .flatMap((c) => c.benchmarks);
  const hasFilters = Boolean(filter.model_id || filter.category || filter.benchmark);

  return (
    <div className="flex flex-col gap-3">
      <div className="grid gap-3 sm:grid-cols-3">
        <SelectFilter label="Category" value={filter.category ?? ""} onChange={set("category")}>
          <option value="">All categories</option>
          {catalog.map((c) => (
            <option key={c.category} value={c.category}>
              {c.title}
            </option>
          ))}
        </SelectFilter>
        <SelectFilter label="Benchmark" value={filter.benchmark ?? ""} onChange={set("benchmark")}>
          <option value="">All benchmarks</option>
          {benchmarks.map((b) => (
            <option key={b.id} value={b.id}>
              {b.name}
            </option>
          ))}
        </SelectFilter>
        <SelectFilter label="Model" value={filter.model_id ?? ""} onChange={set("model_id")}>
          <option value="">All models</option>
          {models.map((m) => (
            <option key={m.id} value={m.id}>
              {m.name}
            </option>
          ))}
        </SelectFilter>
      </div>
      {hasFilters && (
        <Link to="/runs" search={{}} replace resetScroll={false} className="self-start text-xs text-muted underline-offset-2 hover:text-fg hover:underline">
          Clear filters
        </Link>
      )}
    </div>
  );
}
