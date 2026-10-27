import { Badge } from "@/components/ui/Badge";
import type { CategoryCatalog } from "@/lib/api/types";
import { BenchmarkCard } from "./BenchmarkCard";

export function CategorySection({ category }: { category: CategoryCatalog }) {
  const implemented = category.benchmarks.filter((b) => b.implemented).length;
  const headingId = `category-${category.category}`;
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-4">
      <header>
        <div className="flex flex-wrap items-center gap-2">
          <h2 id={headingId} className="text-lg font-semibold">
            {category.title}
          </h2>
          <Badge tone={implemented ? "success" : "neutral"}>
            {implemented}/{category.benchmarks.length} available
          </Badge>
        </div>
        <p className="mt-1 text-sm text-pretty text-muted">{category.description}</p>
      </header>
      <div className="grid items-start gap-4 md:grid-cols-2">
        {category.benchmarks.map((b) => (
          <BenchmarkCard key={b.id} benchmark={b} />
        ))}
      </div>
    </section>
  );
}
