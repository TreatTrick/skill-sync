<script lang="ts">
  import { t } from '@/shared/i18n'
  import { Badge } from '@/shared/ui'
  import type { summarizeSyncSelection } from '../lib/syncStatus'

  type SelectionSummary = ReturnType<typeof summarizeSyncSelection>

  interface Props {
    summary: SelectionSummary
    localStateUpdates: number
    hasLocalStateUpdates: boolean
  }

  let { summary, localStateUpdates, hasLocalStateUpdates }: Props = $props()
</script>

<div class="grid gap-2 border-t border-border pt-4 text-sm">
  <div class="flex flex-wrap items-center justify-between gap-2">
    <span class="font-semibold text-strong-foreground">{t('sync.commitSummary')}</span>
    <Badge variant="secondary">
      {summary.willCreateCommit ? t('sync.commitWillBeCreated') : t('sync.commitNone')}
    </Badge>
  </div>
  <div class="grid gap-1 text-muted-foreground sm:grid-cols-2 lg:grid-cols-5">
    <span>{t('sync.commitSummaryUploads')}: {summary.uploads}</span>
    <span>{t('sync.commitSummaryDownloads')}: {summary.downloads}</span>
    <span>{t('sync.commitSummaryDeleteRemote')}: {summary.deleteRemote}</span>
    <span>{t('sync.commitSummaryDeleteLocal')}: {summary.deleteLocal}</span>
    <span>{t('sync.commitSummaryState')}: {localStateUpdates}</span>
  </div>
  {#if hasLocalStateUpdates && !summary.willCreateCommit}
    <p class="text-xs text-muted-foreground">{t('sync.localBaseOnly')}</p>
  {/if}
</div>
