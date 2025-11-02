import { Button } from '../components/ui/Button'
import { Card } from '../components/ui/Card'

const headlineMetrics = [
  {
    label: 'Total pastes',
    value: '2,847',
    delta: '+12.5%',
    tone: 'text-primary',
  },
  {
    label: 'Encrypted pastes',
    value: '1,126',
    delta: '+4.2%',
    tone: 'text-success-foreground',
  },
  {
    label: 'Burn-after-reading',
    value: '384',
    delta: '-1.9%',
    tone: 'text-warning-foreground',
  },
  {
    label: 'Webhook deliveries',
    value: '96%',
    delta: '+1.2%',
    tone: 'text-info-foreground',
  },
]

const quickActions = [
  {
    title: 'Create secure paste',
    description: 'Start a new encrypted paste with retention defaults tailored for incident response.',
    cta: { label: 'New paste', href: '/' },
  },
  {
    title: 'Invite teammate',
    description: 'Share copypaste.fyi with another engineer and collaborate on shared collections.',
    cta: { label: 'Invite', href: '/settings/team' },
  },
  {
    title: 'View API tokens',
    description: 'Manage personal access tokens and webhook signing secrets.',
    cta: { label: 'Manage tokens', href: '/settings/api' },
  },
]

const recentActivity = [
  {
    title: 'Security runbook v7',
    actor: 'avery@oxide.team',
    timestamp: '5 minutes ago',
    descriptor: 'Shared with Security workspace',
  },
  {
    title: 'On-call notes / week 44',
    actor: 'oncall@oxide.team',
    timestamp: '2 hours ago',
    descriptor: 'Encrypted · Burn after reading',
  },
  {
    title: 'Deploy checklist',
    actor: 'deploy-ci',
    timestamp: 'Yesterday',
    descriptor: 'Webhook · Slack incident room',
  },
]

export const DashboardPage = () => {
  return (
    <div className="space-y-10">
      <header className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
        <div className="space-y-1">
          <h1 className="text-3xl font-semibold text-slate-900 dark:text-slate-100">Welcome back</h1>
          <p className="text-sm text-slate-600 dark:text-slate-400">
            Monitor recent paste activity, encryption adoption, and quick actions for your workspace.
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button onClick={() => (window.location.href = '/')}>New paste</Button>
          <Button variant="outline" onClick={() => (window.location.href = '/stats')}>
            View detailed stats
          </Button>
        </div>
      </header>

      <section className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {headlineMetrics.map((metric) => (
          <Card key={metric.label} padding="lg" className="relative overflow-hidden">
            <div className="space-y-3">
              <p className="text-xs uppercase tracking-wide text-muted-foreground">{metric.label}</p>
              <p className="text-3xl font-semibold text-slate-900 dark:text-slate-100">{metric.value}</p>
              <p className="text-sm font-medium text-success">{metric.delta}</p>
            </div>
          </Card>
        ))}
      </section>

      <section className="grid gap-6 lg:grid-cols-3">
        <Card padding="lg" className="lg:col-span-2">
          <div className="mb-6 flex items-center justify-between">
            <div>
              <h2 className="text-lg font-semibold text-slate-900 dark:text-slate-100">Recent activity</h2>
              <p className="text-sm text-muted-foreground">The latest pastes created across your workspace.</p>
            </div>
            <Button variant="outline" size="sm" onClick={() => (window.location.href = '/pastes')}>
              View all
            </Button>
          </div>
          <div className="space-y-4">
            {recentActivity.map((item) => (
              <div key={item.title} className="flex items-start justify-between gap-4 rounded-xl bg-background/70 px-4 py-3">
                <div className="space-y-1">
                  <p className="text-sm font-semibold text-slate-900 dark:text-slate-100">{item.title}</p>
                  <p className="text-xs text-muted-foreground">
                    {item.actor} · {item.descriptor}
                  </p>
                </div>
                <span className="text-xs text-muted-foreground">{item.timestamp}</span>
              </div>
            ))}
          </div>
        </Card>
        <Card padding="lg">
          <div className="space-y-4">
            <h2 className="text-lg font-semibold text-slate-900 dark:text-slate-100">Quick actions</h2>
            <div className="space-y-4">
              {quickActions.map((action) => (
                <div key={action.title} className="rounded-xl border border-muted/60 bg-background/70 p-4">
                  <p className="text-sm font-semibold text-slate-900 dark:text-slate-100">{action.title}</p>
                  <p className="mt-1 text-xs text-muted-foreground">{action.description}</p>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="mt-3"
                    onClick={() => (window.location.href = action.cta.href)}
                  >
                    {action.cta.label}
                  </Button>
                </div>
              ))}
            </div>
          </div>
        </Card>
      </section>
    </div>
  )
}
