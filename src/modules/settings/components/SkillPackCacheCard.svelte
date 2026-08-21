<script lang="ts">
  import { Trash2 } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import type { CacheStats } from '@/shared/schemas'
  import {
    Button,
    Card,
    CardContent,
    CardDescription,
    CardFooter,
    CardHeader,
    CardTitle,
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

<Card>
  <CardHeader>
    <CardTitle>{t('settings.cache')}</CardTitle>
    <CardDescription>{t('settings.cacheDescription')}</CardDescription>
  </CardHeader>
  <CardContent>
    {#if loading || !stats}
      <p class="text-sm text-muted-foreground">{t('sync.progress.working')}</p>
    {:else}
      <dl class="grid gap-3 text-sm sm:grid-cols-3">
        <div>
          <dt class="text-muted-foreground">{t('settings.cacheEntries')}</dt>
          <dd class="font-medium text-foreground">{stats.entries}</dd>
        </div>
        <div>
          <dt class="text-muted-foreground">{t('settings.cacheSize')}</dt>
          <dd class="font-medium text-foreground">{formatBytes(stats.bytes)}</dd>
        </div>
        <div>
          <dt class="text-muted-foreground">{t('settings.cacheCapacity')}</dt>
          <dd class="font-medium text-foreground">{formatBytes(stats.capacity)}</dd>
        </div>
      </dl>
    {/if}
  </CardContent>
  <CardFooter>
    <Button
      disabled={disabled || clearPending || loading}
      loading={clearPending}
      variant="outline"
      onclick={() => (confirmOpen = true)}
    >
      <Trash2 class="size-4" />
      {t('settings.clearCache')}
    </Button>
  </CardFooter>
</Card>

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
