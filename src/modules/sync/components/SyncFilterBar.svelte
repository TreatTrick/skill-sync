<script lang="ts">
  import { t } from '@/shared/i18n'
  import { Button, Input, Select, SelectContent, SelectItem, SelectTrigger } from '@/shared/ui'
  import {
    SYNC_STATUS_FILTERS,
    type SyncChangeCounts,
    type SyncStatusFilter,
  } from '../lib/syncStatus'

  interface Props {
    search: string
    statusFilter: SyncStatusFilter
    canSelectAll: boolean
    canSelectNone: boolean
    changeCounts: SyncChangeCounts
    totalChanges: number
    onSelectAll: () => void
    onSelectNone: () => void
  }

  let {
    search = $bindable(),
    statusFilter = $bindable(),
    canSelectAll,
    canSelectNone,
    changeCounts,
    totalChanges,
    onSelectAll,
    onSelectNone,
  }: Props = $props()

  const statusFilterLabel = (
    filter: SyncStatusFilter,
  ): `sync.filters.${SyncStatusFilter}` => `sync.filters.${filter}`

  // Only the four pending-change filters carry counts; all/synced never do.
  const changeCountFor = (filter: SyncStatusFilter): number | null => {
    if (filter === 'all' || filter === 'synced') return null
    return changeCounts[filter]
  }
</script>

{#snippet countBadge(count: number)}
  <span
    class="bg-destructive text-destructive-foreground inline-flex h-4 min-w-4 shrink-0 items-center justify-center rounded-full px-1 text-xs font-semibold leading-none"
  >
    {count}
  </span>
{/snippet}

<div class="flex flex-col gap-3 border-b border-border pb-4 sm:flex-row">
  <Input bind:value={search} placeholder={t('sync.searchPlaceholder')} />
  <Select type="single" bind:value={statusFilter}>
    <SelectTrigger aria-label={t('sync.filterLabel')} class="sm:w-52">
      <span class="truncate">{t(statusFilterLabel(statusFilter))}</span>
      {#if totalChanges > 0}
        {@render countBadge(totalChanges)}
      {/if}
    </SelectTrigger>
    <SelectContent>
      {#each SYNC_STATUS_FILTERS as filter (filter)}
        {@const count = changeCountFor(filter)}
        <SelectItem value={filter}>
          <span>{t(statusFilterLabel(filter))}</span>
          {#if count}
            {@render countBadge(count)}
          {/if}
        </SelectItem>
      {/each}
    </SelectContent>
  </Select>
  <div class="flex gap-2">
    <Button
      variant="outline"
      class="flex-1 sm:flex-none"
      onclick={onSelectAll}
      disabled={!canSelectAll}
    >
      {t('sync.selectAll')}
    </Button>
    <Button
      variant="outline"
      class="flex-1 sm:flex-none"
      onclick={onSelectNone}
      disabled={!canSelectNone}
    >
      {t('sync.selectNone')}
    </Button>
  </div>
</div>
