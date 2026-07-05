interface DistributionDatum {
  label: string;
  value: number;
}

interface DistributionCardProps {
  title: string;
  data: DistributionDatum[];
  palette?: "formats" | "encryption" | "default";
}

// Restrained palette: accent, grays, and a single support hue.
const palettes: Record<string, string[]> = {
  formats: [
    "var(--color-accent)",
    "#a1a1aa",
    "#10b981",
    "#71717a",
    "#d4d4d8",
    "#52525b",
  ],
  encryption: ["var(--color-accent)", "#a1a1aa", "#10b981", "#71717a"],
  default: ["var(--color-accent)", "#a1a1aa", "#10b981", "#71717a"],
};

export const DistributionCard = ({
  title,
  data,
  palette = "default",
}: DistributionCardProps) => {
  const total = data.reduce((sum, item) => sum + item.value, 0);
  const colors = palettes[palette] ?? palettes.default;

  if (data.length === 0) {
    return (
      <div>
        <h2 className="text-sm font-semibold tracking-tight text-text">
          {title}
        </h2>
        <p className="mt-4 text-sm text-muted-foreground">
          No data available yet.
        </p>
      </div>
    );
  }

  return (
    <div>
      <h2 className="text-sm font-semibold tracking-tight text-text">
        {title}
      </h2>
      <ul className="mt-4 space-y-3">
        {data.map((item, index) => {
          const percent = total ? Math.round((item.value / total) * 100) : 0;
          const color = colors[index % colors.length];
          return (
            <li key={item.label} className="space-y-1">
              <div className="flex items-center justify-between font-mono text-xs text-muted-foreground">
                <span>{item.label}</span>
                <span className="font-medium text-text">{percent}%</span>
              </div>
              <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
                <div
                  className="h-full rounded-full transition-all"
                  style={{ width: `${percent}%`, backgroundColor: color }}
                  aria-hidden
                />
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
};
