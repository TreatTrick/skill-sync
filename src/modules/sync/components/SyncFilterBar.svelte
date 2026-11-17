<script lang="ts">
  import { Input } from '@/shared/ui'
  import { onMount } from 'svelte'
  import {
    ArrowDownToLine,
    ArrowUpFromLine,
    Check,
    Search,
    Trash2,
    TriangleAlert,
  } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import type { SyncStatusFilter } from '../lib/syncStatus'

  interface Props {
    search: string
    statusFilter: SyncStatusFilter
    counts: Record<SyncStatusFilter, number>
    onStatusFilter: (filter: SyncStatusFilter) => void
  }

  let { search = $bindable(), statusFilter, counts, onStatusFilter }: Props = $props()

  onMount(() => {
    const handleShortcut = (event: KeyboardEvent): void => {
      if (event.key !== '/' || event.metaKey || event.ctrlKey || event.altKey) return
      const target = event.target as HTMLElement | null
      if (target?.matches('input, textarea, select, [contenteditable="true"]')) return
      event.preventDefault()
      document.getElementById('sync-search')?.focus()
    }
    window.addEventListener('keydown', handleShortcut)
    return () => window.removeEventListener('keydown', handleShortcut)
  })

  const filters = [
    { id: 'pending', label: 'sync.filters.pending', icon: null },
    { id: 'conflict', label: 'sync.filters.conflict', icon: TriangleAlert },
    { id: 'delete', label: 'sync.filters.delete', icon: Trash2 },
    { id: 'up', label: 'sync.filters.up', icon: ArrowUpFromLine },
    { id: 'down', label: 'sync.filters.down', icon: ArrowDownToLine },
    { id: 'synced', label: 'sync.filters.synced', icon: Check },
  ] as const
</script>

<div class="toolbar">
  <label class="search">
    <Search class="ic" />
    <Input
      aria-label={t('sync.searchPlaceholder')}
      id="sync-search"
      bind:value={search}
      class="input"
      autocomplete="off"
      placeholder={t('sync.searchPlaceholder')}
      type="search"
    />
    {#if !search}<kbd>/</kbd>{/if}
  </label>
  <div aria-label={t('sync.filterLabel')} class="chips" role="group">
    {#each filters as filter (filter.id)}
      {@const Icon = filter.icon}
      <button
        aria-pressed={statusFilter === filter.id}
        class:zero={counts[filter.id] === 0}
        class="chip"
        data-k={filter.id}
        onclick={() => onStatusFilter(filter.id)}
        type="button"
      >
        {#if Icon}<Icon class="ic" />{/if}
        {t(filter.label)}
        <b>{counts[filter.id]}</b>
      </button>
    {/each}
  </div>
</div>
