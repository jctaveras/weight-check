import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
import type { InvokeArgs } from "@tauri-apps/api/core";
import { mockIPC } from "@tauri-apps/api/mocks";
import { act, render, type RenderOptions } from "@testing-library/react";
import type { ReactElement } from "react";
import { vi } from "vitest";

function makeTestQueryClient() {
  return new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
}

export function renderWithQuery(ui: ReactElement, options?: RenderOptions) {
  const client = makeTestQueryClient();
  return {
    client,
    ...render(<QueryClientProvider client={client}>{ui}</QueryClientProvider>, options),
  };
}

/**
 * Renders `ui` inside a QueryClient and a memory router, for components that use `Link` or
 * `useNavigate`. `ui` stays mounted on every path (a splat route matches anything), so tests
 * can navigate and then assert on `router.state.location`.
 */
export async function renderWithRouter(ui: ReactElement, { path = "/" }: { path?: string } = {}) {
  const client = makeTestQueryClient();
  const rootRoute = createRootRoute({ component: () => ui });
  const splatRoute = createRoute({ getParentRoute: () => rootRoute, path: "$", component: () => null });
  const router = createRouter({
    routeTree: rootRoute.addChildren([splatRoute]),
    history: createMemoryHistory({ initialEntries: [path] }),
  });
  await router.load();
  const result = await act(async () =>
    render(
      <QueryClientProvider client={client}>
        {/* The test router's route tree isn't the app's registered one. */}
        <RouterProvider router={router as never} />
      </QueryClientProvider>,
    ),
  );
  return { client, router, ...result };
}

export type InvokeHandler = (args: Record<string, unknown>) => unknown;

/**
 * Mocks Tauri IPC: each command name maps to a handler that receives the invoke args and
 * returns the result (or throws/rejects to simulate a command error, e.g. `{ code, message }`).
 * Unhandled commands reject with a `not_found` envelope. Returns the spy so tests can inspect
 * `mock.calls` as `[command, args]`. Cleared after each test by `src/test/setup.ts`.
 */
export function mockInvoke(handlers: Record<string, InvokeHandler> = {}) {
  const fn = vi.fn((command: string, args?: InvokeArgs) => {
    const handler = handlers[command];
    if (!handler) throw { code: "not_found", message: `no mock for command ${command}` };
    return handler((args ?? {}) as Record<string, unknown>);
  });
  mockIPC(fn);
  // `isTauri()` reads `globalThis.isTauri`, which mockIPC doesn't set.
  vi.stubGlobal("isTauri", true);
  return fn;
}
