import { Bar, BarChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";

import { formatMetricValue, formatNumber } from "@/lib/format";
import { ChartTooltip } from "./ChartTooltip";
import { axisTick, barRadius, gridStroke, hoverCursor } from "./chartTheme";
import { relativeToBest, type MetricRow, type Series } from "./comparison";

interface OverviewChartProps {
  series: Series[];
  metrics: MetricRow[];
}

/**
 * One group per headline metric, one bar per run, each scaled to the best run (100%) in the
 * metric's own direction, so a longer bar is always better.
 */
export function OverviewChart({ series, metrics }: OverviewChartProps) {
  const data = metrics.map((m) => ({
    metric: m.name,
    ...Object.fromEntries(
      series.map((s, i) => [s.runId, relativeToBest(m.values[i], m.values[m.best!], m.direction)]),
    ),
  }));
  const height = metrics.length * (series.length * 14 + 28) + 40;

  return (
    <div style={{ height }}>
      <ResponsiveContainer width="100%" height="100%">
        <BarChart data={data} layout="vertical" barGap={2} margin={{ top: 0, right: 16, bottom: 0, left: 0 }}>
          <CartesianGrid horizontal={false} stroke={gridStroke} />
          <XAxis
            type="number"
            domain={[0, 100]}
            ticks={[0, 25, 50, 75, 100]}
            tickFormatter={(v: number) => `${v}%`}
            tick={axisTick}
            axisLine={false}
            tickLine={false}
          />
          <YAxis
            type="category"
            dataKey="metric"
            width={160}
            tick={{ ...axisTick, fontFamily: "var(--font-mono)" }}
            axisLine={{ stroke: gridStroke }}
            tickLine={false}
          />
          <Tooltip
            cursor={hoverCursor}
            content={({ active, label }) => {
              const row = metrics.find((m) => m.name === label);
              if (!active || !row) return null;
              return (
                <ChartTooltip
                  title={label}
                  rows={series.map((s, i) => {
                    const value = row.values[i];
                    const pct = relativeToBest(value, row.values[row.best!], row.direction);
                    return {
                      key: s.runId,
                      color: s.color,
                      label: s.label,
                      value:
                        value === undefined
                          ? "—"
                          : `${formatMetricValue(value)} ${row.unit}${pct === undefined ? "" : ` (${formatNumber(pct, 0)}%)`}`,
                    };
                  })}
                />
              );
            }}
          />
          {series.map((s) => (
            <Bar key={s.runId} dataKey={s.runId} name={s.label} fill={s.color} radius={barRadius} barSize={12} isAnimationActive={false} />
          ))}
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}
