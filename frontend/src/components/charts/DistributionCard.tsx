interface DistributionDatum {
  label: string
  value: number
}

interface DistributionCardProps {
  title: string
  data: DistributionDatum[]
  palette?: 'formats' | 'encryption' | 'default'
}

const palettes: Record<string, string[]> = {
  formats: ['#6366f1', '#f97316', '#22d3ee', '#14b8a6', '#8b5cf6', '#ec4899'],
  encryption: ['#6366f1', '#22d3ee', '#14b8a6', '#f97316'],
  default: ['#6366f1', '#22d3ee', '#14b8a6', '#ec4899'],
}

export const DistributionCard = ({ title, data, palette = 'default' }: DistributionCardProps) => {
  const total = data.reduce((sum, item) => sum + item.value, 0)
  const colors = palettes[palette] ?? palettes.default

  if (data.length === 0) {
    return (
      <div className="rounded-2xl border border-slate-800 bg-surface/80 p-6">
        <h2 className="text-lg font-semibold text-gray-100">{title}</h2>
        <p className="mt-4 text-sm text-gray-400">No data available yet.</p>
      </div>
    )
  }

  return (
    <div className="rounded-2xl border border-slate-800 bg-surface/80 p-6">
      <h2 className="text-lg font-semibold text-gray-100">{title}</h2>
      <ul className="mt-4 space-y-3">
        {data.map((item, index) => {
          const percent = total ? Math.round((item.value / total) * 100) : 0
          const color = colors[index % colors.length]
          return (
            <li key={item.label} className="space-y-1">
              <div className="flex items-center justify-between text-sm text-gray-300">
                <span>{item.label}</span>
                <span className="font-medium text-gray-100">{percent}%</span>
              </div>
              <div className="h-2 w-full overflow-hidden rounded-full bg-slate-800">
                <div
                  className="h-full rounded-full transition-all"
                  style={{ width: `${percent}%`, backgroundColor: color }}
                  aria-hidden
                />
              </div>
            </li>
          )
        })}
      </ul>
    </div>
  )
}
