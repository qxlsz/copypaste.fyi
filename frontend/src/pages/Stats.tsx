import { useQuery } from "@tanstack/react-query";
import { fetchStatsSummary } from "../api/client";
import type { StatsSummary } from "../api/types";
import { format } from "date-fns";
import { AreaGroupChart } from "../components/charts/AreaGroupChart";
import { DistributionCard } from "../components/charts/DistributionCard";
import { Button, Card } from "../components/ui";

export const StatsPage = () => {
  const { data, isLoading, isError, error } = useQuery({
    queryKey: ["stats-summary"],
    queryFn: fetchStatsSummary,
  });

  if (isLoading) {
    return (
      <Card padding="lg" className="border-dashed">
        <p className="text-sm text-muted-foreground">Loading stats…</p>
      </Card>
    );
  }

  if (isError) {
    const message = error instanceof Error ? error.message : "Unknown error";
    return (
      <Card padding="lg" className="border-danger/30">
        <p className="text-sm font-medium text-danger">Failed to load stats</p>
        <p className="mt-2 text-xs text-muted-foreground">{message}</p>
      </Card>
    );
  }

  return data ? <StatsContent summary={data} /> : null;
};

interface StatsContentProps {
  summary: StatsSummary;
}

const StatsContent = ({ summary }: StatsContentProps) => {
  return (
    <div className="space-y-8">
      <header className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
        <div className="space-y-1">
          <h1 className="text-xl font-semibold tracking-tight text-text">
            Usage insights
          </h1>
          <p className="text-sm text-muted-foreground">
            Track paste creation trends, encryption adoption, and
            burn-after-reading usage over time.
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => window.print()}>
            Export report
          </Button>
          <Button
            size="sm"
            onClick={() => (window.location.href = "/dashboard")}
          >
            View dashboard
          </Button>
        </div>
      </header>

      <section className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard label="Total pastes" value={summary.totalPastes} />
        <StatCard label="Active" value={summary.activePastes} />
        <StatCard label="Expired" value={summary.expiredPastes} />
        <StatCard
          label="Burn after reading"
          value={summary.burnAfterReadingCount}
        />
      </section>

      <section className="grid gap-6 lg:grid-cols-2">
        <Card padding="lg">
          <DistributionCard
            title="Formats"
            data={summary.formats.map((item) => ({
              label: item.format,
              value: item.count,
            }))}
            palette="formats"
          />
        </Card>
        <Card padding="lg">
          <DistributionCard
            title="Encryption algorithms"
            data={summary.encryptionUsage.map((item) => ({
              label: item.algorithm,
              value: item.count,
            }))}
            palette="encryption"
          />
        </Card>
      </section>

      <Card padding="lg">
        <h2 className="text-sm font-semibold tracking-tight text-text">
          Pastes created over time
        </h2>
        <p className="mb-4 text-xs text-muted-foreground">
          Highlight spikes driven by product launches or campaigns.
        </p>
        <AreaGroupChart
          data={summary.createdByDay.map((item) => ({
            date: item.date,
            value: item.count,
          }))}
          formatLabel={(date) => format(new Date(date), "MMM d")}
        />
      </Card>

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
  );
};

interface StatCardProps {
  label: string;
  value: number;
}

const StatCard = ({ label, value }: StatCardProps) => (
  <Card padding="md">
    <p className="text-xs uppercase tracking-wide text-muted-foreground">
      {label}
    </p>
    <p className="mt-3 font-mono text-3xl font-semibold tracking-tight text-text">
      {value.toLocaleString()}
    </p>
  </Card>
);

interface InsightCardProps {
  title: string;
  description: string;
  value: string;
}

const InsightCard = ({ title, description, value }: InsightCardProps) => (
  <Card padding="lg">
    <h3 className="text-sm font-semibold tracking-tight text-text">{title}</h3>
    <p className="mt-2 text-xs text-muted-foreground">{description}</p>
    <p className="mt-4 font-mono text-sm font-medium text-accent">{value}</p>
  </Card>
);
