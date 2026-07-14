<script lang="ts">
  import type { Component } from 'svelte'

  import { cn } from '@/shared/lib'
  import type { SyncStatusFilter } from '../lib/syncStatus'

  type MetricTone = 'neutral' | 'info' | 'success' | 'warning' | 'destructive'

  interface Props {
    label: string
    value: number | string
    icon: Component<{ class?: string }>
    tone?: MetricTone
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

  const surfaceClasses: Record<MetricTone, string> = {
    neutral: 'border-border bg-surface',
    info: 'border-info-border bg-info-muted',
    success: 'border-success-border bg-success-muted',
    warning: 'border-warning-border bg-warning-muted',
    destructive: 'border-destructive-border bg-destructive-muted',
  }

  const accentClasses: Record<MetricTone, string> = {
    neutral: 'text-strong-foreground',
    info: 'text-info-muted-foreground',
    success: 'text-success',
    warning: 'text-warning',
    destructive: 'text-destructive',
  }
</script>

<button
  type="button"
  class={cn(
    'grid w-full gap-2 rounded-md border p-4 text-left transition-colors disabled:opacity-100',
    surfaceClasses[tone],
    filter ? 'cursor-pointer hover:border-border-strong' : 'cursor-default',
    active ? 'ring-1 ring-primary' : '',
  )}
  aria-pressed={filter ? active : undefined}
  disabled={filter ? undefined : true}
  onclick={filter ? () => onFilter?.(filter) : undefined}
>
  <div class="flex items-center justify-between gap-3">
    <div class="text-sm font-medium text-muted-foreground">{label}</div>
    <span class={accentClasses[tone]}>
      <Icon class="size-4" />
    </span>
  </div>
  <div class={cn('text-3xl font-bold', accentClasses[tone])}>
    {value}
  </div>
</button>
