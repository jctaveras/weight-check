import { Link } from "@tanstack/react-router";

import { NAV_ITEMS } from "./navItems";

const base =
  "rounded-md px-3 py-1.5 text-sm font-medium transition-colors duration-150 hover:bg-surface-muted hover:text-fg";

/** Nav links with the current page marked; Link sets `aria-current="page"` on the active one. */
export function NavLinks() {
  return (
    <ul className="flex flex-wrap items-center gap-1">
      {NAV_ITEMS.map((item) => (
        <li key={item.to}>
          <Link
            to={item.to}
            // "/" matches only itself; other items also match their subpages (/runs/$id).
            activeOptions={{ exact: item.to === "/", includeSearch: false }}
            className={base}
            activeProps={{ className: "bg-surface-muted text-fg", "aria-current": "page" }}
            inactiveProps={{ className: "text-muted" }}
          >
            {item.label}
          </Link>
        </li>
      ))}
    </ul>
  );
}
