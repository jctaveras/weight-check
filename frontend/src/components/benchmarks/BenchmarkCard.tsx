import Link from "next/link";

import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import type { BenchmarkDescriptor } from "@/lib/api/types";
import { RunBenchmarkForm } from "./RunBenchmarkForm";

export function BenchmarkCard({ benchmark }: { benchmark: BenchmarkDescriptor }) {
  return (
    <article className="flex min-w-0 flex-col gap-3 rounded-lg border border-border bg-surface p-5">
      <header className="flex flex-wrap items-start justify-between gap-2">
        <div className="min-w-0">
          <h3 className="font-semibold">{benchmark.name}</h3>
          <p className="truncate font-mono text-xs text-muted" translate="no">
            {benchmark.id}
          </p>
        </div>
        {!benchmark.implemented && <Badge>Not Implemented</Badge>}
      </header>
      <p className="text-sm text-pretty text-muted">{benchmark.description}</p>

      {benchmark.implemented ? (
        <details className="group">
          <summary className="cursor-pointer rounded-md text-sm font-medium text-accent select-none hover:underline">
            Configure &amp; Run
          </summary>
          <div className="mt-3">
            <RunBenchmarkForm benchmark={benchmark} />
          </div>
        </details>
      ) : (
        <div className="flex flex-wrap items-center gap-3">
          <Button disabled>Start Run</Button>
          <span className="text-xs text-muted">Coming in a later release.</span>
        </div>
      )}

      <Link
        href={`/runs?benchmark=${encodeURIComponent(benchmark.id)}`}
        className="mt-auto self-start text-xs text-muted underline-offset-2 hover:text-fg hover:underline"
      >
        View past runs
      </Link>
    </article>
  );
}
