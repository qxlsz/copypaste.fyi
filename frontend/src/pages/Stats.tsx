import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  ApiError,
  fetchStatsSummary,
  fetchTraffic,
  setAdminToken,
  type TrafficSummary,
} from "../api/client";
import type { StatsSummary } from "../api/types";
import { format } from "date-fns";
import { AreaGroupChart } from "../components/charts/AreaGroupChart";
import { DistributionCard } from "../components/charts/DistributionCard";

export const StatsPage = () => {
  const [tokenInput, setTokenInput] = useState("");
  const { data, isLoading, isError, error, refetch } = useQuery({
    queryKey: ["stats-summary"],
    queryFn: fetchStatsSummary,
    retry: false,
  });
  const traffic = useQuery({
    queryKey: ["stats-traffic"],
    queryFn: fetchTraffic,
    retry: false,
  });

  const unauthorized = error instanceof ApiError && (error.status === 401 || error.status === 403);

  if (unauthorized) {
    return (
      <section className="max-w-md space-y-4">
        <h1 className="text-2xl font-medium tracking-tight text-text">Admin only</h1>
        <p className="text-sm leading-relaxed text-muted-foreground">
          Counts live in this server process. They are not sent to a third party. Paste the operator
          token to view them.
        </p>
        <form
          className="space-y-3"
          onSubmit={(event) => {
            event.preventDefault();
            setAdminToken(tokenInput.trim());
            void refetch();
            void traffic.refetch();
          }}
        >
          <input
            type="password"
            value={tokenInput}
            onChange={(event) => setTokenInput(event.target.value)}
            className="h-12 w-full rounded-md border border-border bg-surface px-3 text-sm"
            placeholder="COPYPASTE_ADMIN_TOKEN"
            autoComplete="off"
          />
          <button
            type="submit"
            className="inline-flex h-12 items-center rounded-md bg-accent px-4 text-sm text-accent-foreground"
          >
            Unlock
          </button>
        </form>
      </section>
    );
  }

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
        <h1 className="text-2xl font-medium tracking-tight text-text">Could not load stats</h1>
        <p className="text-sm leading-relaxed text-muted-foreground">{message}</p>
      </section>
    );
  }

  return data ? <StatsContent summary={data} traffic={traffic.data} /> : null;
};

const StatsContent = ({
  summary,
  traffic,
}: {
  summary: StatsSummary;
  traffic?: TrafficSummary;
}) => {
  const encryptedCount = summary.encryptionUsage.reduce((acc, item) => acc + item.count, 0);

  return (
    <div className="mx-auto max-w-3xl space-y-10">
      <header className="space-y-2">
        <h1 className="text-3xl font-medium tracking-tight text-text">This instance</h1>
        <p className="max-w-lg text-sm leading-relaxed text-muted-foreground">
          Live counts on this process. Public copypaste.fyi keeps pastes and visit totals in memory,
          so a deploy resets the figures. No paste ids, no cookies.
        </p>
      </header>

      <section className="grid grid-cols-2 gap-x-6 gap-y-6 sm:grid-cols-4">
        <Stat figure={summary.totalPastes} label="Pastes" />
        <Stat figure={summary.activePastes} label="Active" />
        <Stat figure={summary.expiredPastes} label="Expired" />
        <Stat figure={traffic?.pageviews ?? 0} label="Pageviews" />
      </section>
      {traffic?.startedAt ? (
        <p className="text-xs text-muted-foreground">
          Counting since {format(new Date(traffic.startedAt * 1000), "d MMM yyyy HH:mm")} UTC
        </p>
      ) : null}

      {traffic &&
        (traffic.referrers.length > 0 ||
          traffic.pages.length > 0 ||
          (traffic.oses?.length ?? 0) > 0) && (
          <section className="grid gap-10 lg:grid-cols-3">
            <DistributionCard
              title="Pages"
              data={traffic.pages.map((item) => ({
                label: item.name,
                value: item.count,
              }))}
              palette="formats"
            />
            <DistributionCard
              title="Referrer"
              data={traffic.referrers.map((item) => ({
                label: item.name,
                value: item.count,
              }))}
              palette="encryption"
            />
            <DistributionCard
              title="Device"
              data={traffic.devices.map((item) => ({
                label: item.name,
                value: item.count,
              }))}
              palette="formats"
            />
            <DistributionCard
              title="OS"
              data={(traffic.oses ?? []).map((item) => ({
                label: item.name,
                value: item.count,
              }))}
              palette="encryption"
            />
            <DistributionCard
              title="Country"
              data={(traffic.countries ?? []).map((item) => ({
                label: item.name,
                value: item.count,
              }))}
              palette="formats"
            />
          </section>
        )}
      {traffic && traffic.referrers.length > 0 ? (
        <p className="text-xs text-muted-foreground">
          Referrer is the previous site on that tab. expaste.com is another pastebin. Those hits are
          people opening share links, not creating pastes here. Device and OS come from the
          User-Agent. Country is CF/Vercel/Fly country headers when present, otherwise the browser
          language region. No IPs stored.
        </p>
      ) : null}

      {summary.totalPastes === 0 ? (
        <p className="text-sm text-muted-foreground">No pastes on this instance yet.</p>
      ) : (
        <>
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
            <h2 className="text-base font-medium tracking-tight text-text">Created over time</h2>
            <AreaGroupChart
              data={summary.createdByDay.map((item) => ({
                date: item.date,
                value: item.count,
              }))}
              formatLabel={(date) => format(new Date(date), "MMM d")}
            />
          </section>

          <p className="text-sm text-muted-foreground">
            {encryptedCount.toLocaleString()} encrypted · {summary.timeLockedCount.toLocaleString()}{" "}
            time-locked
          </p>
        </>
      )}
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
