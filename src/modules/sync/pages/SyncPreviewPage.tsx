import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { CheckCircle2, RefreshCw } from 'lucide-react'
import { useState } from 'react'

import { errorMessage } from '@/shared/lib'
import { hostLabel, t } from '@/shared/i18n'
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardHeader,
  EmptyState,
  Spinner,
} from '@/shared/ui'

import { applySyncPlan, getSyncPlan } from '../api/syncApi'
import type { Conflict, SyncAction } from '../schemas/syncPlan'
import { useSyncDecisionsStore } from '../stores/syncDecisionsStore'

const shortHash = (hash: string) =>
  hash.length > 12 ? hash.slice(0, 12) : hash

const directionLabel = (direction: string): string =>
  direction === 'upload'
    ? t('sync.direction.upload')
    : direction === 'download'
      ? t('sync.direction.download')
      : direction

const ActionRow = ({ action }: { action: SyncAction }) => (
  <div className="grid gap-1 rounded-lg border border-border bg-surface p-3 text-sm">
    <div className="flex flex-wrap items-center justify-between gap-2">
      <span className="font-bold text-strong-foreground">{action.name}</span>
      <Badge variant="default">{hostLabel(action.host)}</Badge>
    </div>
    <div className="truncate text-xs text-muted-foreground">
      {directionLabel(action.direction)} · {action.repo_path}
    </div>
  </div>
)

const CHOICES = [
  { key: 'local', labelKey: 'conflicts.keepLocal' },
  { key: 'remote', labelKey: 'conflicts.useRemote' },
  { key: 'skip', labelKey: 'conflicts.skip' },
] as const

const ConflictCard = ({ conflict }: { conflict: Conflict }) => {
  const decision = useSyncDecisionsStore(
    (s) => s.decisions[conflict.skill_id] ?? '',
  )
  const setDecision = useSyncDecisionsStore((s) => s.setDecision)

  return (
    <div className="grid gap-2 rounded-lg border border-warning-border bg-warning-muted p-3 text-sm">
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
}

const GroupSection = ({
  title,
  items,
}: {
  title: string
  items: SyncAction[]
}) => {
  if (items.length === 0) {
    return null
  }
  return (
    <div className="grid gap-2">
      <h3 className="flex items-center gap-2 text-sm font-bold text-strong-foreground">
        {title}
        <Badge variant="default">{items.length}</Badge>
      </h3>
      <div className="grid grid-cols-1 gap-2 lg:grid-cols-2">
        {items.map((action) => (
          <ActionRow action={action} key={action.skill_id} />
        ))}
      </div>
    </div>
  )
}

const ConflictList = ({ conflicts }: { conflicts: Conflict[] }) => {
  if (conflicts.length === 0) {
    return null
  }
  return (
    <div className="grid gap-2">
      <h3 className="flex items-center gap-2 text-sm font-bold text-strong-foreground">
        {t('sync.groups.conflicts')}
        <Badge variant="warning">{conflicts.length}</Badge>
      </h3>
      <div className="grid grid-cols-1 gap-2 lg:grid-cols-2">
        {conflicts.map((conflict) => (
          <ConflictCard conflict={conflict} key={conflict.skill_id} />
        ))}
      </div>
    </div>
  )
}

export const SyncPreviewPage = () => {
  const queryClient = useQueryClient()
  const plan = useQuery({ queryKey: ['sync-plan'], queryFn: getSyncPlan })
  const decisions = useSyncDecisionsStore((state) => state.decisions)
  const clearDecisions = useSyncDecisionsStore((state) => state.clear)
  const [resultMsg, setResultMsg] = useState('')

  const apply = useMutation({
    mutationFn: (vars: Record<string, string>) => applySyncPlan(vars),
    onSuccess: (data) => {
      setResultMsg(
        t('sync.applied', {
          count: data.applied.length,
          backups: data.backups.length,
        }),
      )
      clearDecisions()
      void queryClient.invalidateQueries({ queryKey: ['sync-plan'] })
    },
    onError: (error) =>
      setResultMsg(t('sync.applyError', { message: errorMessage(error) })),
  })

  const planData = plan.data
  const totalActions = planData
    ? planData.uploads.length +
      planData.downloads.length +
      planData.updates.length +
      planData.deletes.length
    : 0
  const conflictCount = planData?.conflicts.length ?? 0
  const isEmpty = totalActions === 0 && conflictCount === 0

  const handleApply = () => {
    setResultMsg('')
    apply.mutate(decisions)
  }

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader
          action={
            <div className="flex gap-2">
              <Button
                icon={<RefreshCw className="size-4" />}
                onClick={() => void plan.refetch()}
                variant="secondary"
              >
                {t('sync.recheck')}
              </Button>
              <Button
                disabled={isEmpty}
                loading={apply.isPending}
                onClick={handleApply}
              >
                {apply.isPending
                  ? t('sync.applying')
                  : t('common.actions.apply')}
              </Button>
            </div>
          }
          description={t('sync.description')}
          title={t('sync.title')}
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
            {t('sync.loadError', { message: errorMessage(plan.error) })}
          </CardBody>
        </Card>
      ) : null}

      {resultMsg ? (
        <Card className="border-success-muted bg-success-muted">
          <CardBody className="flex items-center gap-2 text-sm text-success">
            <CheckCircle2 className="size-4 shrink-0" />
            {resultMsg}
          </CardBody>
        </Card>
      ) : null}

      {isEmpty && !plan.isLoading && !plan.error ? (
        <Card>
          <EmptyState
            icon={<CheckCircle2 className="size-10" />}
            title={t('sync.empty')}
          />
        </Card>
      ) : null}

      {planData ? (
        <>
          <GroupSection
            items={planData.uploads}
            title={t('sync.groups.uploads')}
          />
          <GroupSection
            items={planData.downloads}
            title={t('sync.groups.downloads')}
          />
          <GroupSection
            items={planData.updates}
            title={t('sync.groups.updates')}
          />
          <GroupSection
            items={planData.deletes}
            title={t('sync.groups.deletes')}
          />
          <ConflictList conflicts={planData.conflicts} />
        </>
      ) : null}
    </div>
  )
}
