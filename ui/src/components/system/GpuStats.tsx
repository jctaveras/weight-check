import { EmptyState } from "@/components/ui/EmptyState";
import { Meter } from "@/components/ui/Meter";
import type { GpuSnapshot } from "@/lib/api/types";
import { formatBytes, formatNumber } from "@/lib/format";

export function GpuStats({ gpu }: { gpu: GpuSnapshot | null }) {
  if (!gpu) {
    return (
      <EmptyState title="No GPU Telemetry">
        Only AMD GPUs (amdgpu sysfs) are monitored right now.
      </EmptyState>
    );
  }
  const vramPct = gpu.vram_total_bytes ? (gpu.vram_used_bytes / gpu.vram_total_bytes) * 100 : null;
  return (
    <div className="flex min-w-0 flex-col gap-4">
      <div className="min-w-0">
        <h3 className="text-xs tracking-wide text-muted uppercase">GPU</h3>
        <p className="truncate text-sm font-medium" title={gpu.name} translate="no">
          {gpu.name}
        </p>
        <p className="text-xs text-muted tabular-nums">
          {gpu.power_w === null ? "Power n/a" : `${formatNumber(gpu.power_w, 0)} W`}
        </p>
      </div>
      <Meter label="GPU Busy" percent={gpu.busy_pct} />
      <Meter
        label="VRAM"
        percent={vramPct}
        detail={`${formatBytes(gpu.vram_used_bytes)} / ${formatBytes(gpu.vram_total_bytes)}`}
      />
    </div>
  );
}
