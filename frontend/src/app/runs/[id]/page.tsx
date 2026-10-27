import type { Metadata } from "next";
import { Suspense } from "react";

import { RunDetailRoute } from "@/components/runs/RunDetailRoute";
import { LoadingText } from "@/components/ui/LoadingText";

export const metadata: Metadata = { title: "Run" };

export default function RunPage() {
  return (
    // The run id is only known at request time, so useParams suspends during prerendering.
    <Suspense fallback={<LoadingText>Loading run…</LoadingText>}>
      <RunDetailRoute />
    </Suspense>
  );
}
