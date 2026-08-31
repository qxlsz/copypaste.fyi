import { useQuery } from "@tanstack/react-query";
import { fetchStatsSummary } from "../api/client";
import type { StatsSummary } from "../api/types";
import { format } from "date-fns";
import { AreaGroupChart } from "../components/charts/AreaGroupChart";
import { DistributionCard } from "../components/charts/DistributionCard";

export const StatsPage = () => {
  const { data, isLoading, isError, error } = useQuery({
    queryKey: ["stats-summary"],
    queryFn: fetchStatsSummary,
  });

  if (isLoading) {
    return (
      <div
        className="flex min-h-[40vh] items-center justify-center"
        role="status"
        aria-label="Loading stats"
      >
        <span className="h-5 w-5 animate-spin rounded-full border-2 border-border border-t-accent" />
      </div>
    );
  }

  if (isError) {
    const message = error instanceof Error ? error.message : "Unknown error";
    return (
      <section className="max-w-md space-y-2">
        <h1 className="text-2xl font-medium tracking-tight text-text">
          Could not load stats
        </h1>
        <p className="text-sm leading-relaxed text-muted-foreground">{message}</p>
      </section>
    );
  }

  return data ? <StatsContent summary={data} /> : null;
};

const StatsContent = ({ summary }: { summary: StatsSummary }) => {
  const encryptedCount = summary.encryptionUsage.reduce(
    (acc, item) => acc + item.count,
    0,
  );

  return (
    <div className="mx-auto max-w-3xl space-y-10">
      <header className="space-y-2">
        <h1 className="text-3xl font-medium tracking-tight text-text">
          This instance
        </h1>
        <p className="max-w-lg text-sm leading-relaxed text-muted-foreground">
          Counts for pastes created here. There is still no public listing of
          individual pastes.
        </p>
      </header>

      <section className="grid grid-cols-2 gap-x-6 gap-y-6 sm:grid-cols-4">
        <Stat figure={summary.totalPastes} label="Total" />
        <Stat figure={summary.activePastes} label="Active" />
        <Stat figure={summary.expiredPastes} label="Expired" />
        <Stat figure={summary.burnAfterReadingCount} label="Burn" />
      </section>

      <section className="grid gap-10 lg:grid-cols-2">
        <DistributionCard
          title="Formats"
          data={summary.formats.map((item) => ({
            label: item.format,
            value: item.count,
          }))}
          palette="formats"
        />
        <DistributionCard
          title="Encryption"
          data={summary.encryptionUsage.map((item) => ({
            label: item.algorithm,
            value: item.count,
          }))}
          palette="encryption"
        />
      </section>

      <section className="space-y-3">
        <h2 className="text-base font-medium tracking-tight text-text">
          Created over time
        </h2>
        <AreaGroupChart
          data={summary.createdByDay.map((item) => ({
            date: item.date,
            value: item.count,
          }))}
          formatLabel={(date) => format(new Date(date), "MMM d")}
        />
      </section>

      <p className="text-sm text-muted-foreground">
        {encryptedCount.toLocaleString()} encrypted ·{" "}
        {summary.timeLockedCount.toLocaleString()} time-locked
      </p>
    </div>
  );
};

const Stat = ({ figure, label }: { figure: number; label: string }) => (
  <div>
    <p className="font-mono text-3xl font-medium tracking-tight text-text">
      {figure.toLocaleString()}
    </p>
    <p className="mt-1 text-xs text-muted-foreground">{label}</p>
  </div>
);
