import { useState } from "react";
import { Bar, BarChart, CartesianGrid, Cell, LabelList, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";

import { SelectField } from "@/components/ui/SelectField";
import { formatMetricValue } from "@/lib/format";
import { ChartTooltip } from "./ChartTooltip";
import { axisTick, barRadius, gridStroke, hoverCursor, truncateLabel } from "./chartTheme";
import type { MetricRow, Series } from "./comparison";

const DIRECTION_NOTE = {
  higher: "Higher is better.",
  lower: "Lower is better.",
  neutral: "Reflects the run's settings; neither direction is better.",
};

interface MetricChartProps {
  series: Series[];
  metrics: MetricRow[];
  /** Metric shown first, usually the benchmark's headline metric. */
  initial?: string;
}

/** Absolute values of one picked metric, one bar per run, labelled at the bar end. */
export function MetricChart({ series, metrics, initial }: MetricChartProps) {
  const [picked, setPicked] = useState(initial);
  const row = metrics.find((m) => m.name === picked) ?? metrics[0];
  if (!row) return null;

  const data = series.flatMap((s, i) => {
    const value = row.values[i];
    if (value === undefined) return [];
    const display = `${formatMetricValue(value)} ${row.unit}${row.best === i ? " · Best" : ""}`;
    return [{ runId: s.runId, label: s.label, color: s.color, value, display }];
  });

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-end gap-x-4 gap-y-2">
        <div className="w-full sm:w-72">
          <SelectField label="Metric" value={row.name} onChange={setPicked}>
            {metrics.map((m) => (
              <option key={m.name} value={m.name}>
                {m.name} ({m.unit})
              </option>
            ))}
          </SelectField>
        </div>
        <p className="pb-2.5 text-sm text-muted">{DIRECTION_NOTE[row.direction]}</p>
      </div>
      <div style={{ height: data.length * 36 + 40 }}>
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={data} layout="vertical" margin={{ top: 0, right: 120, bottom: 0, left: 0 }}>
            <CartesianGrid horizontal={false} stroke={gridStroke} />
            <XAxis type="number" domain={[0, "auto"]} tick={axisTick} axisLine={false} tickLine={false} tickFormatter={(v: number) => formatMetricValue(v)} />
            <YAxis
              type="category"
              dataKey="label"
              width={200}
              tick={axisTick}
              tickFormatter={(v: string) => truncateLabel(v)}
              axisLine={{ stroke: gridStroke }}
              tickLine={false}
            />
            <Tooltip
              cursor={hoverCursor}
              content={({ active, payload }) => {
                const item = payload?.[0]?.payload as (typeof data)[number] | undefined;
                if (!active || !item) return null;
                return (
                  <ChartTooltip
                    title={row.name}
                    rows={[{ key: item.runId, color: item.color, label: item.label, value: item.display }]}
                  />
                );
              }}
            />
            <Bar dataKey="value" radius={barRadius} barSize={20} isAnimationActive={false}>
              {data.map((d) => (
                <Cell key={d.runId} fill={d.color} />
              ))}
              <LabelList dataKey="display" position="right" fill="var(--fg)" fontSize={12} />
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
