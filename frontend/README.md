# weight-check frontend

Next.js 16 (App Router, TypeScript, Tailwind CSS v4, TanStack Query) UI for the weight-check backend. You can use it to load and unload models, start benchmarks, and watch runs complete with their metrics.

```sh
pnpm install
cp .env.example .env.local   # optional; defaults to http://127.0.0.1:8787
pnpm dev                     # http://localhost:3000 (the backend must be running)
pnpm lint && pnpm typecheck && pnpm test && pnpm build
```

The browser calls the backend directly at `NEXT_PUBLIC_API_URL`. The backend allows CORS, and a model load can take longer than the Next dev proxy timeout, so requests don't go through Next.

## Layout

```
src/app/          thin route pages (/, /models, /benchmarks, /runs, /runs/[id])
src/components/   ui/ primitives, plus layout/, system/, models/, benchmarks/, runs/ feature components
src/lib/api/      typed fetch client (ApiError) and one function per endpoint; types mirror the Rust types
src/lib/queries/  TanStack Query hooks (polling, mutations, cache invalidation)
src/lib/          format.ts (Intl formatting) and errors.ts (error code → message + next step)
```

Tests are `*.test.ts(x)` files next to the code they cover, run with Vitest + Testing Library (jsdom).
