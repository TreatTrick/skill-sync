import { useQuery } from '@tanstack/react-query'
import { ListChecks } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import { errorMessage } from '@/shared/lib'
import { t } from '@/shared/i18n'
import {
  Badge,
  Card,
  CardBody,
  CardHeader,
  EmptyState,
  Spinner,
} from '@/shared/ui'
import { getSyncPlan, useSyncDecisionsStore } from '@/modules/sync'

const shortHash = (hash: string) =>
  hash.length > 12 ? hash.slice(0, 12) : hash

const CHOICES = [
  { key: 'local', labelKey: 'conflicts.keepLocal' },
  { key: 'remote', labelKey: 'conflicts.useRemote' },
  { key: 'skip', labelKey: 'conflicts.skip' },
] as const

export const ConflictsPage = () => {
  // Re-render on i18n language change (react-router data router isolates parent re-renders).
  useTranslation()
  const plan = useQuery({ queryKey: ['sync-plan'], queryFn: getSyncPlan })
  const decisions = useSyncDecisionsStore((state) => state.decisions)
  const setDecision = useSyncDecisionsStore((state) => state.setDecision)

  const conflicts = plan.data?.conflicts ?? []

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader
          description={t('conflicts.description')}
          title={t('conflicts.title')}
        />
      </Card>

      {plan.isLoading ? (
        <div className="flex justify-center py-12">
          <Spinner className="size-6" />
        </div>
      ) : null}

      {plan.error ? (
        <Card className="border-destructive-border bg-destructive-muted">
          <CardBody className="text-sm text-destructive">
            {errorMessage(plan.error)}
          </CardBody>
        </Card>
      ) : null}

      <p className="text-xs text-muted-foreground">
        {t('conflicts.reviewAtSync')}
      </p>

      {conflicts.length === 0 && !plan.isLoading ? (
        <Card>
          <EmptyState
            icon={<ListChecks className="size-10" />}
            title={t('conflicts.empty')}
          />
        </Card>
      ) : null}

      <div className="grid grid-cols-1 gap-2 lg:grid-cols-2">
        {conflicts.map((conflict) => {
          const decision = decisions[conflict.skill_id] ?? ''
          return (
            <div
              className="grid gap-2 rounded-lg border border-warning-border bg-warning-muted p-3 text-sm"
              key={conflict.skill_id}
            >
              <div className="flex flex-wrap items-center justify-between gap-2">
                <span className="font-bold text-strong-foreground">
                  {conflict.name}
                </span>
                <Badge variant="warning">{conflict.reason}</Badge>
              </div>
              <div className="grid grid-cols-1 gap-1 text-xs text-muted-foreground sm:grid-cols-2">
                <div className="truncate">
                  {t('conflicts.localHash')}: {shortHash(conflict.local_hash)}
                </div>
                <div className="truncate">
                  {t('conflicts.remoteHash')}: {shortHash(conflict.remote_hash)}
                </div>
              </div>
              <div className="flex flex-wrap gap-2">
                {CHOICES.map((choice) => (
                  <button
                    className={[
                      'h-8 rounded-lg border px-2.5 text-xs font-medium transition-colors',
                      decision === choice.key
                        ? 'border-primary bg-primary-muted text-primary-muted-foreground'
                        : 'border-border bg-surface text-foreground hover:bg-surface-hover',
                    ].join(' ')}
                    key={choice.key}
                    onClick={() => setDecision(conflict.skill_id, choice.key)}
                    type="button"
                  >
                    {t(choice.labelKey)}
                  </button>
                ))}
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}
