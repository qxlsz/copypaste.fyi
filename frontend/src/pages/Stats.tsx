import { useQuery } from '@tanstack/react-query'
import { fetchStatsSummary } from '../api/client'
import type { StatsSummary } from '../api/types'
import { format } from 'date-fns'
import { AreaGroupChart } from '../components/charts/AreaGroupChart'
import { DistributionCard } from '../components/charts/DistributionCard'

export const StatsPage = () => {
  const { data, isLoading, isError, error } = useQuery({
    queryKey: ['stats-summary'],
    queryFn: fetchStatsSummary,
  })

  if (isLoading) {
    return <p className="text-gray-300">Loading stats…</p>
  }

  if (isError) {
    const message = error instanceof Error ? error.message : 'Unknown error'
    return <p className="text-danger">Failed to load stats: {message}</p>
  }

  return data ? <StatsContent summary={data} /> : null
}

interface StatsContentProps {
  summary: StatsSummary
}

const StatsContent = ({ summary }: StatsContentProps) => {
  return (
    <div className="space-y-10">
      <header className="space-y-2">
        <h1 className="text-3xl font-semibold text-gray-100">Usage insights</h1>
        <p className="text-gray-400">
          Track paste creation trends, encryption adoption, and burn-after-reading usage over time.
        </p>
      </header>

      <section className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard label="Total pastes" value={summary.totalPastes} accent="text-primary" />
        <StatCard label="Active" value={summary.activePastes} accent="text-success" />
        <StatCard label="Expired" value={summary.expiredPastes} accent="text-danger" />
        <StatCard label="Burn after reading" value={summary.burnAfterReadingCount} accent="text-accent" />
      </section>

      <section className="grid gap-8 lg:grid-cols-2">
        <DistributionCard
          title="Formats"
          data={summary.formats.map((item) => ({ label: item.format, value: item.count }))}
          palette="formats"
        />
        <DistributionCard
          title="Encryption algorithms"
          data={summary.encryptionUsage.map((item) => ({ label: item.algorithm, value: item.count }))}
          palette="encryption"
        />
      </section>

      <section className="rounded-2xl border border-slate-800 bg-surface/80 p-6">
        <h2 className="text-lg font-semibold text-gray-100">Pastes created over time</h2>
        <p className="mb-4 text-sm text-gray-400">
          Highlight spikes driven by product launches or campaigns.
        </p>
        <AreaGroupChart
          data={summary.createdByDay.map((item) => ({ date: item.date, value: item.count }))}
          formatLabel={(date) => format(new Date(date), 'MMM d')}
        />
      </section>

      <section className="grid gap-6 lg:grid-cols-2">
        <InsightCard
          title="Encryption adoption"
          description="See how many pastes leverage client-side encryption. Encourage secure defaults when usage is low."
          value={`${summary.encryptionUsage.reduce((acc, item) => acc + item.count, 0)} encrypted pastes`}
        />
        <InsightCard
          title="Time-locked pastes"
          description="Ensure that time-bound links are being used for sensitive disclosures."
          value={`${summary.timeLockedCount} pastes have a viewing window`}
        />
      </section>
    </div>
  )
}

interface StatCardProps {
  label: string
  value: number
  accent?: string
}

const StatCard = ({ label, value, accent }: StatCardProps) => (
  <div className="rounded-2xl border border-slate-800 bg-surface/80 p-5 shadow shadow-surface/40">
    <p className="text-xs uppercase tracking-wide text-gray-400">{label}</p>
    <p className={`mt-2 text-3xl font-semibold ${accent ?? 'text-gray-100'}`}>{value.toLocaleString()}</p>
  </div>
)

interface InsightCardProps {
  title: string
  description: string
  value: string
}

const InsightCard = ({ title, description, value }: InsightCardProps) => (
  <div className="rounded-2xl border border-slate-800 bg-surface/80 p-6">
    <h3 className="text-lg font-semibold text-gray-100">{title}</h3>
    <p className="mt-2 text-sm text-gray-400">{description}</p>
    <p className="mt-4 text-xl font-semibold text-accent">{value}</p>
  </div>
)
