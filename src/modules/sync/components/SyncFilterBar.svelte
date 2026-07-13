<script lang="ts">
  import { t } from '@/shared/i18n'
  import { Button, Input, Select, SelectContent, SelectItem, SelectTrigger } from '@/shared/ui'
  import { SYNC_STATUS_FILTERS, type SyncStatusFilter } from '../lib/syncStatus'

  interface Props {
    search: string
    statusFilter: SyncStatusFilter
    canSelectAll: boolean
    canSelectNone: boolean
    onSelectAll: () => void
    onSelectNone: () => void
  }

  let {
    search = $bindable(),
    statusFilter = $bindable(),
    canSelectAll,
    canSelectNone,
    onSelectAll,
    onSelectNone,
  }: Props = $props()

  const statusFilterLabel = (
    filter: SyncStatusFilter,
  ): `sync.filters.${SyncStatusFilter}` => `sync.filters.${filter}`
</script>

<div class="flex flex-col gap-3 border-b border-border pb-4 sm:flex-row">
  <Input bind:value={search} placeholder={t('sync.searchPlaceholder')} />
  <Select type="single" bind:value={statusFilter}>
    <SelectTrigger aria-label={t('sync.filterLabel')} class="sm:w-52">
      {t(statusFilterLabel(statusFilter))}
    </SelectTrigger>
    <SelectContent>
      {#each SYNC_STATUS_FILTERS as filter (filter)}
        <SelectItem value={filter}>{t(statusFilterLabel(filter))}</SelectItem>
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
