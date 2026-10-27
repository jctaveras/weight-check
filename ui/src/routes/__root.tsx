import { createRootRoute, Link, Outlet } from "@tanstack/react-router";

import { AppShell } from "@/components/layout/AppShell";
import { DocumentTitle } from "@/components/layout/DocumentTitle";
import { EmptyState } from "@/components/ui/EmptyState";

export const Route = createRootRoute({
  component: RootLayout,
  notFoundComponent: NotFound,
});

function RootLayout() {
  return (
    <AppShell>
      <DocumentTitle />
      <Outlet />
    </AppShell>
  );
}

function NotFound() {
  return (
    <EmptyState title="Page Not Found">
      This page doesn&apos;t exist.{" "}
      <Link to="/" className="font-medium text-accent underline underline-offset-2">
        Go to the dashboard
      </Link>
      .
    </EmptyState>
  );
}
