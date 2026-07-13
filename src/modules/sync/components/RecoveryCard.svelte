<script lang="ts">
  import { t } from '@/shared/i18n'
  import type { RecoveryInfo } from '@/shared/schemas'
  import { Button, Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/shared/ui'

  interface Props {
    recovery: RecoveryInfo
    resumePending: boolean
    onResume: () => void
  }

  let { recovery, resumePending, onResume }: Props = $props()
</script>

<Card class="border-warning-border bg-warning-muted">
  <CardHeader>
    <CardTitle>{t('sync.recoveryRequired')}</CardTitle>
    <CardDescription>{recovery.message}</CardDescription>
  </CardHeader>
  <CardContent class="grid gap-3 text-sm">
    <div class="grid gap-1 text-muted-foreground sm:grid-cols-3">
      <span>{t('sync.recoveryPhase')}: {recovery.phase}</span>
      <span>{t('sync.recoveryCompletedCount')}: {recovery.completed_action_ids.length}</span>
      <span>{t('sync.recoveryPendingCount')}: {recovery.pending_action_ids.length}</span>
    </div>
    <div class="flex justify-end">
      <Button
        disabled={resumePending}
        loading={resumePending}
        onclick={onResume}
      >
        {t('sync.resumeRecovery')}
      </Button>
    </div>
  </CardContent>
</Card>
