<script lang="ts">
  import { Trash2 } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import type { CacheStats } from '@/shared/schemas'
  import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
  } from '@/shared/ui'

  interface Props {
    stats: CacheStats | undefined
    loading: boolean
    clearPending: boolean
    disabled: boolean
    onClear: () => void
  }

  let { stats, loading, clearPending, disabled, onClear }: Props = $props()
  let confirmOpen = $state(false)

  const formatBytes = (value: number): string => {
    if (value >= 1024 * 1024 * 1024) return `${(value / 1024 / 1024 / 1024).toFixed(1)} GiB`
    if (value >= 1024 * 1024) return `${Math.round(value / 1024 / 1024)} MiB`
    if (value >= 1024) return `${Math.round(value / 1024)} KiB`
    return `${value} B`
  }

  const confirmClear = (): void => {
    confirmOpen = false
    onClear()
  }
</script>
<div class="srows"><div class="srow">
  <div class="lab">
    {#if loading || !stats}<small>{t('sync.progress.working')}</small>
    {:else}
      <div class="meter">
        <div aria-label={t('settings.cacheUsage')} aria-valuemax={stats.capacity} aria-valuemin="0" aria-valuenow={Math.min(stats.bytes, stats.capacity)} class="bar" role="progressbar">
          <i style:width={Math.min(stats.capacity > 0 ? stats.bytes / stats.capacity * 100 : 0, 100) + '%'}></i>
        </div>
        <small><span class="num">{t('settings.cacheUsed', { count: stats.entries, size: formatBytes(stats.bytes) })}</span><span class="num">{t('settings.cacheLimit', { size: formatBytes(stats.capacity) })}</span></small>
      </div>
    {/if}
  </div>
  <div class="ctl"><Button class="btn" disabled={disabled || clearPending || loading || !stats?.entries} loading={clearPending} variant="outline" onclick={() => (confirmOpen = true)}><Trash2 class="size-4" />{t('settings.clearCache')}</Button></div>
</div></div>
<Dialog bind:open={confirmOpen}>
  <DialogContent>
    <DialogHeader>
      <DialogTitle>{t('settings.clearCacheTitle')}</DialogTitle>
      <DialogDescription>{t('settings.clearCacheConfirm')}</DialogDescription>
    </DialogHeader>
    <DialogFooter>
      <Button variant="outline" onclick={() => (confirmOpen = false)}>
        {t('common.actions.cancel')}
      </Button>
      <Button variant="destructive" onclick={confirmClear}>
        {t('settings.clearCache')}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
