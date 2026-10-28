import { formatPercent } from "@/lib/format";

interface MeterProps {
  label: string;
  /** Value on a 0–100 scale, or null when unavailable. */
  percent: number | null;
  /** Text shown on the right, e.g. "12.3 GiB / 24 GiB". */
  detail?: string;
}

export function Meter({ label, percent, detail }: MeterProps) {
  const clamped = percent === null ? 0 : Math.min(100, Math.max(0, percent));
  const tone = clamped >= 90 ? "bg-danger" : clamped >= 70 ? "bg-accent" : "bg-success";
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <div className="flex items-baseline justify-between gap-3 text-sm">
        <span className="font-medium">{label}</span>
        <span className="truncate text-muted tabular-nums">
          {detail ?? formatPercent(percent)}
        </span>
      </div>
      <div
        role="meter"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent === null ? undefined : Math.round(clamped)}
        aria-valuetext={detail ?? formatPercent(percent)}
        className="h-2 overflow-hidden rounded-full bg-surface-muted"
      >
        <div
          className={`h-full origin-left rounded-full transition-transform duration-500 ${tone}`}
          style={{ transform: `scaleX(${clamped / 100})` }}
        />
      </div>
    </div>
  );
}
