import { useQuery } from '@tanstack/react-query'
import { Link } from 'react-router-dom'

import { errorMessage } from '@/shared/lib'
import { t } from '@/shared/i18n'
import { getSyncPlan } from '@/modules/sync'
import { scanSkills } from '@/modules/skills'
import { getAppState } from '@/modules/settings'

interface MetricProps {
  label: string
  value: number | string
  tone?: 'neutral' | 'warning' | 'destructive'
}

const MetricCard = ({ label, value, tone = 'neutral' }: MetricProps) => {
  const valueClass =
    tone === 'warning'
      ? 'text-warning'
      : tone === 'destructive'
        ? 'text-destructive'
        : 'text-strong-foreground'
  return (
    <div className="rounded-lg border border-border bg-surface p-4">
      <div className="text-sm font-medium text-muted-foreground">{label}</div>
      <div className={`mt-3 text-2xl font-bold ${valueClass}`}>{value}</div>
    </div>
  )
}

export const DashboardPage = () => {
  const state = useQuery({ queryKey: ['app-state'], queryFn: getAppState })
  const configured = state.data?.configured ?? false
  const scan = useQuery({
    queryKey: ['scan-skills'],
    queryFn: scanSkills,
    enabled: configured,
  })
  const plan = useQuery({
    queryKey: ['sync-plan'],
    queryFn: getSyncPlan,
    enabled: configured,
  })

  if (state.isLoading) {
    return (
      <p className="text-sm text-muted-foreground">
        {t('common.status.loading')}
      </p>
    )
  }

  if (state.error) {
    return (
      <p className="rounded-lg border border-destructive-border bg-destructive-muted p-3 text-sm text-destructive">
        {errorMessage(state.error)}
      </p>
    )
  }

  if (!configured) {
    return (
      <div className="grid gap-3 rounded-lg border border-border bg-surface p-4">
        <p className="text-sm text-muted-foreground">
          {t('dashboard.notConfigured')}
        </p>
        <Link
          className="inline-flex h-9 w-fit items-center rounded-lg bg-primary px-3 text-sm font-bold text-primary-foreground"
          to="/app/onboarding"
        >
          {t('dashboard.goToOnboarding')}
        </Link>
      </div>
    )
  }

  const discovered = scan.data?.skills.length ?? 0
  const uploads = plan.data?.uploads.length ?? 0
  const downloads = plan.data?.downloads.length ?? 0
  const conflicts = plan.data?.conflicts.length ?? 0

  return (
    <section className="grid gap-4">
      <div className="flex flex-col justify-between gap-3 rounded-lg border border-border bg-surface p-4 sm:flex-row sm:items-center">
        <div>
          <h2 className="text-lg font-bold text-strong-foreground">
            {t('dashboard.title')}
          </h2>
          <p className="mt-1 text-sm text-muted-foreground">
            {t('dashboard.description')}
          </p>
        </div>
        <Link
          className="inline-flex h-9 items-center justify-center gap-2 rounded-lg bg-primary px-3 text-sm font-bold text-primary-foreground"
          to="/app/sync"
        >
          {t('dashboard.preview')}
        </Link>
      </div>

      {state.data && !state.data.git_available ? (
        <p className="rounded-lg border border-warning-border bg-warning-muted p-3 text-sm text-warning">
          {t('dashboard.gitUnavailable')}
        </p>
      ) : null}

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          label={t('dashboard.metrics.discovered')}
          value={discovered}
        />
        <MetricCard label={t('dashboard.metrics.toUpload')} value={uploads} />
        <MetricCard
          label={t('dashboard.metrics.toDownload')}
          value={downloads}
        />
        <MetricCard
          label={t('dashboard.metrics.conflicts')}
          tone={conflicts > 0 ? 'warning' : 'neutral'}
          value={conflicts}
        />
      </div>

      {plan.error ? (
        <p className="rounded-lg border border-warning-border bg-warning-muted p-3 text-sm text-warning">
          {t('sync.loadError', { message: errorMessage(plan.error) })}
        </p>
      ) : null}
    </section>
  )
}
