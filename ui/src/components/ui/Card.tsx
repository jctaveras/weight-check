import type { ReactNode } from "react";

interface CardProps {
  title?: ReactNode;
  /** Heading level for the title; keep the page's heading hierarchy intact. */
  as?: "h2" | "h3";
  actions?: ReactNode;
  className?: string;
  children: ReactNode;
}

export function Card({ title, as: Heading = "h2", actions, className = "", children }: CardProps) {
  return (
    <section className={`min-w-0 rounded-lg border border-border bg-surface p-5 ${className}`}>
      {(title || actions) && (
        <header className="mb-4 flex flex-wrap items-center justify-between gap-3">
          {title && <Heading className="text-base font-semibold">{title}</Heading>}
          {actions && <div className="flex items-center gap-2">{actions}</div>}
        </header>
      )}
      {children}
    </section>
  );
}
