<script lang="ts">
  import { t } from '@/shared/i18n'
  import { Button } from '@/shared/ui'

  interface Props {
    selectedCount: number
    willCreateCommit: boolean
    showCommitHint: boolean
    canApply: boolean
    applyPending: boolean
    onApply: () => void
  }

  let {
    selectedCount,
    willCreateCommit,
    showCommitHint,
    canApply,
    applyPending,
    onApply,
  }: Props = $props()
</script>

<div class="sticky bottom-0 z-10 mt-auto -mx-4 border-t border-border bg-background/85 px-4 py-3 backdrop-blur sm:-mx-6 sm:px-6">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <span class="text-sm text-muted-foreground">
      {t('sync.selectedCount', { count: selectedCount })}
      {#if showCommitHint}
        · {willCreateCommit ? t('sync.commitWillBeCreated') : t('sync.commitNone')}
      {/if}
    </span>
    <Button disabled={!canApply} loading={applyPending} onclick={onApply}>
      {t('common.actions.apply')}
    </Button>
  </div>
</div>
