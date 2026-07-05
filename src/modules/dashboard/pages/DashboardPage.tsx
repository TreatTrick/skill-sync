import { useQuery } from '@tanstack/react-query'
import {
  AlertTriangle,
  ArrowDownToLine,
  ArrowRight,
  ArrowUpFromLine,
  Package,
  Sparkles,
} from 'lucide-react'
import type { ReactNode } from 'react'
import { useNavigate } from 'react-router-dom'

import { errorMessage } from '@/shared/lib'
import { t } from '@/shared/i18n'
import { getSyncPlan } from '@/modules/sync'
import { scanSkills } from '@/modules/skills'
import { getAppState } from '@/modules/settings'
import { Button, Card, CardBody, EmptyState, Spinner } from '@/shared/ui'

interface MetricProps {
  label: string
  value: number | string
  icon: ReactNode
  tone?: 'neutral' | 'warning'
}

const MetricCard = ({ label, value, icon, tone = 'neutral' }: MetricProps) => (
  <Card className="p-4">
    <div className="flex items-center justify-between gap-3">
      <div className="text-sm font-medium text-muted-foreground">{label}</div>
      <span
        className={
          tone === 'warning' ? 'text-warning' : 'text-muted-foreground'
        }
      >
        {icon}
      </span>
    </div>
    <div
      className={`mt-3 text-3xl font-bold ${
        tone === 'warning' ? 'text-warning' : 'text-strong-foreground'
      }`}
    >
      {value}
    </div>
  </Card>
)

export const DashboardPage = () => {
  const navigate = useNavigate()
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
      <div className="flex justify-center py-12">
        <Spinner className="size-6" />
      </div>
    )
  }

  if (state.error) {
    return (
      <Card>
        <CardBody>
          <p className="text-sm text-destructive">
            {errorMessage(state.error)}
          </p>
        </CardBody>
      </Card>
    )
  }

  if (!configured) {
    return (
      <Card>
        <EmptyState
          action={
            <Button
              icon={<Sparkles className="size-4" />}
              onClick={() => navigate('/app/onboarding')}
            >
              {t('dashboard.goToOnboarding')}
            </Button>
          }
          icon={<Sparkles className="size-10" />}
          title={t('dashboard.notConfigured')}
        />
      </Card>
    )
  }

  const conflictCount = plan.data?.conflicts.length ?? 0

  return (
    <div className="grid gap-4">
      <Card>
        <CardBody className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
          <div>
            <h2 className="text-lg font-bold text-strong-foreground">
              {t('dashboard.title')}
            </h2>
            <p className="mt-1 text-sm text-muted-foreground">
              {t('dashboard.description')}
            </p>
          </div>
          <Button
            icon={<ArrowRight className="size-4" />}
            onClick={() => navigate('/app/sync')}
          >
            {t('dashboard.preview')}
          </Button>
        </CardBody>
      </Card>

      {state.data && !state.data.git_available ? (
        <Card>
          <CardBody className="flex items-center gap-2 text-sm text-warning">
            <AlertTriangle className="size-4 shrink-0" />
            {t('dashboard.gitUnavailable')}
          </CardBody>
        </Card>
      ) : null}

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          icon={<Package className="size-4" />}
          label={t('dashboard.metrics.discovered')}
          value={scan.data?.skills.length ?? 0}
        />
        <MetricCard
          icon={<ArrowUpFromLine className="size-4" />}
          label={t('dashboard.metrics.toUpload')}
          value={plan.data?.uploads.length ?? 0}
        />
        <MetricCard
          icon={<ArrowDownToLine className="size-4" />}
          label={t('dashboard.metrics.toDownload')}
          value={plan.data?.downloads.length ?? 0}
        />
        <MetricCard
          icon={<AlertTriangle className="size-4" />}
          label={t('dashboard.metrics.conflicts')}
          tone={conflictCount > 0 ? 'warning' : 'neutral'}
          value={conflictCount}
        />
      </div>

      {plan.error ? (
        <Card>
          <CardBody className="text-sm text-warning">
            {t('sync.loadError', { message: errorMessage(plan.error) })}
          </CardBody>
        </Card>
      ) : null}
    </div>
  )
}
