"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { NAV_ITEMS, isCurrent } from "./navItems";

const base =
  "rounded-md px-3 py-1.5 text-sm font-medium transition-colors duration-150 hover:bg-surface-muted hover:text-fg";

/** Nav links with the current page marked; `pathname` is null in the static fallback. */
export function NavLinkList({ pathname }: { pathname: string | null }) {
  return (
    <ul className="flex flex-wrap items-center gap-1">
      {NAV_ITEMS.map((item) => {
        const current = pathname !== null && isCurrent(pathname, item.href);
        return (
          <li key={item.href}>
            <Link
              href={item.href}
              aria-current={current ? "page" : undefined}
              className={`${base} ${current ? "bg-surface-muted text-fg" : "text-muted"}`}
            >
              {item.label}
            </Link>
          </li>
        );
      })}
    </ul>
  );
}

export function NavLinks() {
  return <NavLinkList pathname={usePathname()} />;
}
