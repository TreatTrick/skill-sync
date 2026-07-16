<script lang="ts">
  import { ArrowDownToLine, ArrowUpFromLine, Trash2 } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import { Button } from '@/shared/ui'

  import type { ConflictBias, SyncStatusFilter } from '../lib/syncStatus'

  interface Props {
    statusFilter: SyncStatusFilter
    visibleCount: number
    onBulkConflict: (bias: ConflictBias) => void
    onBulkDelete: (action: 'delete' | 'recover') => void
  }

  let {
    statusFilter,
    visibleCount,
    onBulkConflict,
    onBulkDelete,
  }: Props = $props()

  const disabled = $derived(visibleCount === 0)
</script>

{#if statusFilter === 'conflict'}
  <div class="flex flex-wrap gap-2">
    <Button
      variant="outline"
      size="sm"
      class="border-remote-border bg-remote-muted text-remote hover:bg-remote-muted/80"
      {disabled}
      onclick={() => onBulkConflict('remote')}
    >
      <ArrowDownToLine class="size-4" />
      {t('sync.bulk.useAllRemote')}
    </Button>
    <Button
      variant="outline"
      size="sm"
      class="border-success-border bg-success-muted text-success hover:bg-success-muted/80"
      {disabled}
      onclick={() => onBulkConflict('local')}
    >
      <ArrowUpFromLine class="size-4" />
      {t('sync.bulk.keepAllLocal')}
    </Button>
  </div>
{:else if statusFilter === 'delete_remote'}
  <div class="flex flex-wrap gap-2">
    <Button
      variant="destructive"
      size="sm"
      {disabled}
      onclick={() => onBulkDelete('delete')}
    >
      <Trash2 class="size-4" />
      {t('sync.bulk.deleteAllRemote')}
    </Button>
    <Button
      variant="outline"
      size="sm"
      class="border-success-border bg-success-muted text-success hover:bg-success-muted/80"
      {disabled}
      onclick={() => onBulkDelete('recover')}
    >
      <ArrowDownToLine class="size-4" />
      {t('sync.bulk.restoreAllRemote')}
    </Button>
  </div>
{:else if statusFilter === 'delete_local'}
  <div class="flex flex-wrap gap-2">
    <Button
      variant="destructive"
      size="sm"
      {disabled}
      onclick={() => onBulkDelete('delete')}
    >
      <Trash2 class="size-4" />
      {t('sync.bulk.deleteAllLocal')}
    </Button>
    <Button
      variant="outline"
      size="sm"
      class="border-success-border bg-success-muted text-success hover:bg-success-muted/80"
      {disabled}
      onclick={() => onBulkDelete('recover')}
    >
      <ArrowUpFromLine class="size-4" />
      {t('sync.bulk.keepAllLocal')}
    </Button>
  </div>
{/if}
