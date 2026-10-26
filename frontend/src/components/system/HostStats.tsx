import { Meter } from "@/components/ui/Meter";
import type { HostSnapshot } from "@/lib/api/types";
import { formatBytes } from "@/lib/format";

export function HostStats({ host }: { host: HostSnapshot }) {
  const ramPct = host.ram_total_bytes ? (host.ram_used_bytes / host.ram_total_bytes) * 100 : null;
  return (
    <div className="flex min-w-0 flex-col gap-4">
      <div className="min-w-0">
        <h3 className="text-xs tracking-wide text-muted uppercase">CPU</h3>
        <p className="truncate text-sm font-medium" title={host.cpu} translate="no">
          {host.cpu}
        </p>
        <p className="text-xs text-muted tabular-nums">{host.cpu_cores} cores</p>
      </div>
      <Meter label="CPU Usage" percent={host.cpu_usage_pct} />
      <Meter
        label="RAM"
        percent={ramPct}
        detail={`${formatBytes(host.ram_used_bytes)} / ${formatBytes(host.ram_total_bytes)}`}
      />
      {host.process_rss_bytes !== null && (
        <p className="text-sm text-muted">
          llama-server RSS:{" "}
          <span className="text-fg tabular-nums">{formatBytes(host.process_rss_bytes)}</span>
        </p>
      )}
    </div>
  );
}
