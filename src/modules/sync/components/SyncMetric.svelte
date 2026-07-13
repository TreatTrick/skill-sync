<script lang="ts">
  import type { Component } from 'svelte'

  import { cn } from '@/shared/lib'
  import type { SyncStatusFilter } from '../lib/syncStatus'

  interface Props {
    label: string
    value: number | string
    icon: Component<{ class?: string }>
    tone?: 'neutral' | 'warning'
    filter?: SyncStatusFilter
    activeFilter?: SyncStatusFilter
    onFilter?: (filter: SyncStatusFilter) => void
  }

  let {
    label,
    value,
    icon: Icon,
    tone = 'neutral',
    filter,
    activeFilter,
    onFilter,
  }: Props = $props()

  const active = $derived(filter !== undefined && activeFilter === filter)
</script>

<button
  type="button"
  class={cn(
    'grid w-full gap-2 rounded-md border bg-surface p-4 text-left transition-colors disabled:opacity-100',
    filter ? 'cursor-pointer hover:border-border-strong' : 'cursor-default border-border',
    active ? 'border-primary ring-1 ring-primary' : '',
  )}
  aria-pressed={filter ? active : undefined}
  disabled={filter ? undefined : true}
  onclick={filter ? () => onFilter?.(filter) : undefined}
>
  <div class="flex items-center justify-between gap-3">
    <div class="text-sm font-medium text-muted-foreground">{label}</div>
    <span class={tone === 'warning' ? 'text-warning' : 'text-muted-foreground'}>
      <Icon class="size-4" />
    </span>
  </div>
  <div class={cn('text-3xl font-bold', tone === 'warning' ? 'text-warning' : 'text-strong-foreground')}>
    {value}
  </div>
</button>
