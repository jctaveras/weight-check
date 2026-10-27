"use client";

import Link from "next/link";

import { Spinner } from "@/components/ui/Spinner";
import { useModelStatus } from "@/lib/queries/useModelStatus";

/** Compact header indicator of backend reachability and the loaded model. */
export function ModelStatusPill() {
  const { data, isError, isPending } = useModelStatus();

  let dot = "bg-muted";
  let text = "Connecting…";
  if (isError) {
    dot = "bg-danger";
    text = "Backend Offline";
  } else if (data?.loading_model_id) {
    text = "Loading Model…";
  } else if (data?.loaded) {
    dot = "bg-success";
    text = data.loaded.model.name;
  } else if (data) {
    dot = "bg-border";
    text = "No Model Loaded";
  }

  return (
    <Link
      href="/models"
      className="flex max-w-64 min-w-0 items-center gap-2 rounded-full border border-border px-3 py-1 text-xs text-muted transition-colors duration-150 hover:text-fg"
    >
      {data?.loading_model_id || isPending ? (
        <Spinner className="size-3" />
      ) : (
        <span aria-hidden="true" className={`size-2 shrink-0 rounded-full ${dot}`} />
      )}
      <span className="truncate" aria-live="polite" translate="no">
        {text}
      </span>
    </Link>
  );
}
