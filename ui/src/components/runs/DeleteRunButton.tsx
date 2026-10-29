import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/Button";
import { ErrorMessage } from "@/components/ui/ErrorMessage";
import { useDeleteRun } from "@/lib/queries/useDeleteRun";

interface DeleteRunButtonProps {
  runId: string;
  /** Called once the run is gone, e.g. to leave its detail page. */
  onDeleted?: () => void;
  size?: "sm" | "md";
}

/** Delete with an inline confirmation step: Delete Run → Confirm Delete / Keep Run. */
export function DeleteRunButton({ runId, onDeleted, size = "sm" }: DeleteRunButtonProps) {
  const [confirming, setConfirming] = useState(false);
  const del = useDeleteRun();
  const group = useRef<HTMLDivElement>(null);
  const opened = useRef(false);

  // Move focus to Confirm Delete when it appears, and back to Delete Run when dismissed.
  useEffect(() => {
    if (confirming) opened.current = true;
    if (!opened.current) return;
    const target = confirming ? "[data-confirm]" : "[data-delete]";
    group.current?.querySelector<HTMLButtonElement>(target)?.focus();
  }, [confirming]);

  const cancel = () => {
    setConfirming(false);
    del.reset();
  };

  return (
    <div className="flex flex-col items-end gap-2">
      <div
        ref={group}
        className="flex items-center gap-2"
        onKeyDown={(e) => {
          if (e.key === "Escape" && confirming) cancel();
        }}
      >
        {confirming ? (
          <>
            <span className="sr-only" aria-live="polite">
              Confirm deleting this run. It can't be undone.
            </span>
            <Button
              data-confirm
              variant="danger"
              size={size}
              loading={del.isPending}
              onClick={() => del.mutate(runId, { onSuccess: () => onDeleted?.() })}
            >
              {del.isPending ? "Deleting…" : "Confirm Delete"}
            </Button>
            <Button variant="secondary" size={size} disabled={del.isPending} onClick={cancel}>
              Keep Run
            </Button>
          </>
        ) : (
          <Button data-delete variant="ghost" size={size} onClick={() => setConfirming(true)}>
            Delete Run
          </Button>
        )}
      </div>
      {del.error && <ErrorMessage error={del.error} title="Couldn't delete this run" />}
    </div>
  );
}
