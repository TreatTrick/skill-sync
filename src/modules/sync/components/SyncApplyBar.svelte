<script lang="ts">
  import { ArrowDownToLine, ArrowUpFromLine, GitCommitHorizontal, Trash2, TriangleAlert } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import type { SyncProgressEvent } from '@/shared/schemas'
  import { Button, Checkbox } from '@/shared/ui'
  import type { summarizeSyncSelection } from '../lib/syncStatus'

  type SelectionSummary = ReturnType<typeof summarizeSyncSelection>

  interface Props {
    summary: SelectionSummary
    showCommitHint: boolean
    canApply: boolean
    applyPending: boolean
    progress: SyncProgressEvent | null
    phaseLabel: string
    deleteGuardTripped: boolean
    deleteGuardAck: boolean
    localStateUpdates: number
    onDeleteGuardAck: (ack: boolean) => void
    onApply: () => void
  }

  let {
    summary,
    showCommitHint,
    canApply,
    applyPending,
    progress,
    phaseLabel,
    deleteGuardTripped,
    deleteGuardAck,
    localStateUpdates,
    onDeleteGuardAck,
    onApply,
  }: Props = $props()
  const percent = $derived(progress?.determinate && progress.total
    ? Math.min(100, Math.round(progress.current / progress.total * 100)) : 0)
</script>

<footer class="applybar">
  <div class="in">
    {#if applyPending}
      <div class="prog" aria-live="polite">
        <div class="line">
          <span><b>{phaseLabel}</b>{#if progress?.total} <span class="num muted">{progress.current}/{progress.total}</span>{/if}{#if progress?.skill_id} <span class="mono text-xs muted">{progress.skill_id}</span>{/if}</span>
          <span class="muted">{t('sync.progress.keepOpen')}</span>
        </div>
        <div class="bar" role="progressbar" aria-label={phaseLabel} aria-valuemin="0" aria-valuemax="100" aria-valuenow={progress?.determinate ? percent : undefined}><i style:width={(progress?.determinate ? percent : 33) + '%'}></i></div>
      </div>
    {:else}
    <div class="sum">
      <div class="l1">
        <strong>{summary.selected ? t('sync.selectedCount', { count: summary.selected }) : t('sync.noneSelected')}</strong>
        {#if summary.uploads > 0}<span class="pill up"><ArrowUpFromLine class="size-3.5" />{t('sync.commitSummaryUploads')} {summary.uploads}</span>{/if}
        {#if summary.downloads > 0}<span class="pill down"><ArrowDownToLine class="size-3.5" />{t('sync.commitSummaryDownloads')} {summary.downloads}</span>{/if}
        {#if summary.deleteRemote > 0}<span class="pill del"><Trash2 class="size-3.5" />{t('sync.commitSummaryDeleteRemote')} {summary.deleteRemote}</span>{/if}
        {#if summary.deleteLocal > 0}<span class="pill del"><Trash2 class="size-3.5" />{t('sync.commitSummaryDeleteLocal')} {summary.deleteLocal}</span>{/if}
      </div>
      <div class="l2">
        {#if showCommitHint}
          <span><GitCommitHorizontal class="ic sm" />{summary.willCreateCommit ? t('sync.commitWillBeCreated') : t('sync.commitNone')}</span>
        {/if}
        {#if localStateUpdates > 0}<span>{t('sync.localBasePlanned', { count: localStateUpdates })}</span>{/if}
        {#if summary.unresolvedConflicts > 0}<span class="warn"><TriangleAlert class="size-3.5" />{t('sync.unresolvedConflicts', { count: summary.unresolvedConflicts })}</span>{/if}
        {#if summary.skipped > 0}<span>{t('sync.skippedCount', { count: summary.skipped })}</span>{/if}
      </div>
    </div>
    <div class="right">
      {#if deleteGuardTripped && summary.hasDelete}
        <label class="ack"><Checkbox checked={deleteGuardAck} onCheckedChange={(checked) => onDeleteGuardAck(checked === true)} />{t('sync.deleteGuard.confirm', { count: summary.deleteRemote + summary.deleteLocal })}</label>
      {/if}
      <Button class="btn primary lg" disabled={!canApply || applyPending} loading={applyPending} onclick={onApply}>
        {t('common.actions.apply')}{#if summary.selected > 0} · {summary.selected} {t('sync.selectedUnit')}{/if}
      </Button>
    </div>
    {/if}
  </div>
</footer>
