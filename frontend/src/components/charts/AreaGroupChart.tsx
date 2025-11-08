import { useMemo } from "react";
import {
  area as d3Area,
  curveMonotoneX,
  extent,
  max,
  scaleLinear,
  scaleTime,
  type ScaleLinear,
  type ScaleTime,
} from "d3";

interface AreaGroupChartProps {
  data: Array<{ date: string; value: number }>;
  formatLabel?: (date: Date) => string;
  height?: number;
}

interface ChartComputed {
  pathData: string;
  gradientId: string;
  xTicks: Array<{ tick: Date; x: number }>;
  yTicks: Array<{ tick: number; y: number }>;
  xScale: ScaleTime<number, number>;
  yScale: ScaleLinear<number, number>;
  width: number;
  margin: { top: number; right: number; bottom: number; left: number };
}

export const AreaGroupChart = ({
  data,
  formatLabel,
  height = 240,
}: AreaGroupChartProps) => {
  const computed = useMemo<ChartComputed | null>(() => {
    if (data.length === 0) {
      return null;
    }

    const parsed = data.map((item) => ({
      date: new Date(item.date),
      value: item.value,
    }));
    const [minDate, maxDate] = extent(parsed, (d) => d.date) as [Date, Date];
    const maxValue = Math.max(1, max(parsed, (d) => d.value) ?? 0);

    const width = 640;
    const margin = { top: 16, right: 16, bottom: 32, left: 40 };

    const xScale = scaleTime<number, number>()
      .domain([minDate, maxDate])
      .range([margin.left, width - margin.right]);

    const yScale = scaleLinear<number, number>()
      .domain([0, maxValue])
      .range([height - margin.bottom, margin.top]);

    const areaGenerator = d3Area<{ date: Date; value: number }>()
      .x((d) => xScale(d.date))
      .y0(() => yScale(0))
      .y1((d) => yScale(d.value))
      .curve(curveMonotoneX);

    const xTickValues = xScale.ticks(6);
    const yTickValues = yScale.ticks(4);

    return {
      pathData: areaGenerator(parsed) ?? "",
      gradientId: `area-gradient-${Math.random().toString(36).slice(2)}`,
      xTicks: xTickValues.map((tick) => ({ tick, x: xScale(tick) })),
      yTicks: yTickValues.map((tick) => ({ tick, y: yScale(tick) })),
      xScale,
      yScale,
      width,
      margin,
    };
  }, [data, height]);

  if (!computed || !computed.pathData) {
    return <p className="text-sm text-gray-400">No data available yet.</p>;
  }

  const { pathData, gradientId, xTicks, yTicks, width, margin } = computed;

  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      role="img"
      aria-label="Pastes created over time"
      className="w-full"
    >
      <defs>
        <linearGradient id={gradientId} x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stopColor="#6366f1" stopOpacity={0.6} />
          <stop offset="100%" stopColor="#6366f1" stopOpacity={0.05} />
        </linearGradient>
      </defs>
      <path
        d={pathData}
        fill={`url(#${gradientId})`}
        stroke="#6366f1"
        strokeWidth={2}
      />

      {xTicks.map(({ tick, x }) => (
        <g
          key={`x-${tick.toISOString()}`}
          transform={`translate(${x}, ${height - margin.bottom})`}
        >
          <line x1={0} x2={0} y1={0} y2={6} stroke="#334155" />
          <text
            dy={16}
            textAnchor="middle"
            className="fill-gray-400 text-[10px]"
          >
            {formatLabel ? formatLabel(tick) : tick.toLocaleDateString()}
          </text>
        </g>
      ))}

      {yTicks.map(({ tick, y }) => (
        <g key={`y-${tick}`} transform={`translate(${margin.left}, ${y})`}>
          <line
            x1={0}
            x2={width - margin.left - margin.right}
            y1={0}
            y2={0}
            stroke="#1e293b"
          />
          <text
            x={-8}
            dy={4}
            textAnchor="end"
            className="fill-gray-400 text-[10px]"
          >
            {tick}
          </text>
        </g>
      ))}
    </svg>
  );
};
