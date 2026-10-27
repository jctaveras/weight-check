import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { buttonClasses } from "@/components/ui/buttonClasses";
import { Card } from "@/components/ui/Card";
import { EmptyState } from "@/components/ui/EmptyState";
import { ErrorMessage } from "@/components/ui/ErrorMessage";
import { LoadingText } from "@/components/ui/LoadingText";
import type { ModelStatus } from "@/lib/api/types";
import { formatBytes, formatDuration, formatNumber } from "@/lib/format";
import { useModelStatus } from "@/lib/queries/useModelStatus";
import { UnloadButton } from "./UnloadButton";

function Stat({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="min-w-0">
      <dt className="text-xs tracking-wide text-muted uppercase">{label}</dt>
      <dd className="truncate text-sm font-medium tabular-nums">{children}</dd>
    </div>
  );
}

function idleText(status: ModelStatus): string {
  if (status.active_runs > 0) return `${status.active_runs} active run${status.active_runs === 1 ? "" : "s"}`;
  if (status.idle_timeout_secs === null) return `Idle ${formatDuration(status.idle_secs * 1000)}`;
  const left = Math.max(0, status.idle_timeout_secs - status.idle_secs);
  return `Unloads in ${formatDuration(left * 1000)}`;
}

interface ActiveModelCardProps {
  /** Show a "Manage" link to /models (used on the dashboard). */
  showManageLink?: boolean;
}

export function ActiveModelCard({ showManageLink = false }: ActiveModelCardProps) {
  const { data: status, error, isPending } = useModelStatus();
  const loaded = status?.loaded;

  return (
    <Card
      title="Active Model"
      actions={
        <>
          {showManageLink && (
            <Link to="/models" className={buttonClasses("ghost", "sm")}>
              Manage
            </Link>
          )}
          {loaded && <UnloadButton modelName={loaded.model.name} />}
        </>
      }
    >
      {isPending ? (
        <LoadingText>Checking model status…</LoadingText>
      ) : error && !status ? (
        <ErrorMessage error={error} title="Couldn't read model status" />
      ) : status?.loading_model_id ? (
        <LoadingText>Starting llama-server…</LoadingText>
      ) : loaded ? (
        <div className="flex flex-col gap-4">
          <div className="min-w-0">
            <p className="truncate text-lg font-semibold" title={loaded.model.name} translate="no">
              {loaded.model.name}
            </p>
            <p className="text-sm text-muted" aria-live="polite">
              {idleText(status)}
            </p>
          </div>
          <dl className="grid grid-cols-2 gap-4 sm:grid-cols-4">
            <Stat label="Context">{formatNumber(loaded.settings.ctx_size)}</Stat>
            <Stat label="GPU Layers">
              {loaded.settings.n_gpu_layers >= 999 ? "All" : loaded.settings.n_gpu_layers}
            </Stat>
            <Stat label="Parallel">{loaded.settings.parallel}</Stat>
            <Stat label="Threads">{loaded.settings.threads ?? "Auto"}</Stat>
            <Stat label="Load Time">{formatDuration(loaded.load_time_ms)}</Stat>
            <Stat label="VRAM Δ">
              {loaded.vram_after_load_bytes !== null && loaded.vram_before_load_bytes !== null
                ? formatBytes(loaded.vram_after_load_bytes - loaded.vram_before_load_bytes)
                : "—"}
            </Stat>
            <Stat label="PID">{loaded.pid ?? "—"}</Stat>
            <Stat label="Quant">{loaded.model.quantization ?? "—"}</Stat>
          </dl>
        </div>
      ) : (
        <EmptyState title="No Model Loaded">
          {showManageLink ? (
            <>
              <Link to="/models" className="font-medium text-accent underline underline-offset-2">
                Load a model
              </Link>{" "}
              to start benchmarking.
            </>
          ) : (
            "Choose a model and load it to start benchmarking."
          )}
        </EmptyState>
      )}
    </Card>
  );
}
