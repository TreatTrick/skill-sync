<script lang="ts">
  import { Check, Clock3, TriangleAlert } from '@lucide/svelte'
  import { t } from '@/shared/i18n'
  import type { RecoveryInfo } from '@/shared/schemas'
  import { Button } from '@/shared/ui'
  interface Props { recovery: RecoveryInfo; resumePending: boolean; onResume: () => void }
  let { recovery, resumePending, onResume }: Props = $props()
  const phaseKeys = {
    remote_outcome_unknown: 'sync.recoveryPhases.remote_outcome_unknown',
    local_replace_failed: 'sync.recoveryPhases.local_replace_failed',
    state_save_failed: 'sync.recoveryPhases.state_save_failed',
  } as const
  const completed = $derived(recovery.completed_action_ids.length)
  const total = $derived(completed + recovery.pending_action_ids.length)
  const percent = $derived(total ? Math.round(completed / total * 100) : 0)
</script>
<header class="sync-head"><div><h1>{t('sync.recoveryRequired')}</h1><div class="meta"><span>{t('sync.recoveryHint')}</span></div></div></header>
<div class="recovery" role="alert">
  <h2><TriangleAlert class="ic lg" />{t(phaseKeys[recovery.phase])}</h2>
  <p>{recovery.message}</p>
  <div class="steps">
    <div class="done"><Check class="ic" />{t('sync.recoveryCompletedCount', { count: completed })}{#if recovery.remote_commit}<span>{t('sync.recoveryCommit')} <span class="mono">{recovery.remote_commit.slice(0, 7)}</span></span>{/if}</div>
    <div class="todo"><Clock3 class="ic" />{t('sync.recoveryPendingCount', { count: recovery.pending_action_ids.length })}</div>
  </div>
  <div class="bar" role="progressbar" aria-label={t('sync.recoveryPhase')} aria-valuemin="0" aria-valuemax="100" aria-valuenow={percent}><i class="recovery-progress" style:width={percent + '%'}></i></div>
  <div class="acts"><Button class="btn primary" disabled={resumePending} loading={resumePending} onclick={onResume}>{t('sync.resumeRecovery')}</Button><span class="muted text-xs">{t('sync.recoveryPhase')}：{t(phaseKeys[recovery.phase])}</span></div>
</div>
