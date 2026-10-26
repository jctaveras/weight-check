import Link from "next/link";
import { Suspense, type ReactNode } from "react";

import { ModelStatusPill } from "./ModelStatusPill";
import { NavLinkList, NavLinks } from "./NavLinks";

export function AppShell({ children }: { children: ReactNode }) {
  return (
    <>
      <a
        href="#main"
        className="sr-only z-50 rounded-md bg-accent px-3 py-2 text-accent-fg focus:not-sr-only focus:fixed focus:top-3 focus:left-3"
      >
        Skip to Content
      </a>
      <header className="border-b border-border bg-surface">
        <div className="mx-auto flex max-w-6xl flex-wrap items-center gap-x-6 gap-y-3 px-4 py-3 sm:px-6">
          <Link href="/" className="text-base font-semibold tracking-tight" translate="no">
            weight<span className="text-accent">-</span>check
          </Link>
          <nav aria-label="Main" className="min-w-0 flex-1">
            {/* usePathname suspends under dynamic routes (/runs/[id]) with Cache Components. */}
            <Suspense fallback={<NavLinkList pathname={null} />}>
              <NavLinks />
            </Suspense>
          </nav>
          <ModelStatusPill />
        </div>
      </header>
      <main id="main" tabIndex={-1} className="mx-auto outline-none w-full max-w-6xl flex-1 px-4 py-8 sm:px-6">
        {children}
      </main>
    </>
  );
}
