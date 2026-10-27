import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from "@tanstack/react-router";
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

export function jsonResponse(body: unknown, status = 200) {
  return new Response(status === 204 ? null : JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

/** Stubs global fetch; returns the mock so tests can inspect calls. */
export function mockFetch(handler: (url: string, init?: RequestInit) => Response | Promise<Response>) {
  const fn = vi.fn((input: RequestInfo | URL, init?: RequestInit) => Promise.resolve(handler(String(input), init)));
  vi.stubGlobal("fetch", fn);
  return fn;
}
