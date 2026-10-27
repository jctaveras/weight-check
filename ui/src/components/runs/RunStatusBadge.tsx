import { Badge, type Tone } from "@/components/ui/Badge";
import { Spinner } from "@/components/ui/Spinner";
import type { RunStatus } from "@/lib/api/types";

const STATUS: Record<RunStatus, { label: string; tone: Tone }> = {
  queued: { label: "Queued", tone: "neutral" },
  running: { label: "Running", tone: "info" },
  completed: { label: "Completed", tone: "success" },
  failed: { label: "Failed", tone: "danger" },
};

export function RunStatusBadge({ status }: { status: RunStatus }) {
  const { label, tone } = STATUS[status];
  return (
    <Badge tone={tone}>
      {status === "running" && <Spinner className="size-3" />}
      {label}
    </Badge>
  );
}
