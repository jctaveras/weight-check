"use client";

import { useState } from "react";

import { Button } from "@/components/ui/Button";
import { ErrorMessage } from "@/components/ui/ErrorMessage";
import { useUnloadModel } from "@/lib/queries/useUnloadModel";

/** Two-step unload: the first click asks for confirmation inline, no browser dialog. */
export function UnloadButton({ modelName }: { modelName: string }) {
  const [confirming, setConfirming] = useState(false);
  const unload = useUnloadModel();

  return (
    <div className="flex flex-col items-end gap-2">
      {confirming ? (
        <div className="flex flex-wrap items-center justify-end gap-2" role="group" aria-label="Confirm unload">
          <span className="text-sm text-muted">Unload {modelName}?</span>
          <Button
            size="sm"
            variant="danger"
            loading={unload.isPending}
            onClick={() => unload.mutate(undefined, { onSettled: () => setConfirming(false) })}
          >
            {unload.isPending ? "Unloading…" : "Unload"}
          </Button>
          <Button size="sm" variant="ghost" disabled={unload.isPending} onClick={() => setConfirming(false)}>
            Cancel
          </Button>
        </div>
      ) : (
        <Button
          size="sm"
          variant="secondary"
          onClick={() => {
            unload.reset();
            setConfirming(true);
          }}
        >
          Unload Model
        </Button>
      )}
      {unload.error && <ErrorMessage error={unload.error} title="Couldn't unload" />}
    </div>
  );
}
