<script lang="ts">
  import type { Component } from 'svelte'

  import { cn } from '@/shared/lib'
  import type { SyncStatusFilter } from '../lib/syncStatus'

  type MetricTone = 'neutral' | 'info' | 'success' | 'warning' | 'destructive' | 'destructiveSoft' | 'primary'

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
  const isZero = $derived(value === 0)
  // Only risk-carrying buckets (deletes, conflicts) tint the number, and only
  // when they actually hold work; everything else stays calm so the metric row
  // reads as one quiet system instead of a wall of saturated color.
  const attention = $derived(
    tone === 'warning' || tone === 'destructive' || tone === 'destructiveSoft',
  )

  // Color lives in the small icon chip, never in a full-card fill.
  const chipClasses: Record<MetricTone, string> = {
    neutral: 'bg-surface-muted text-muted-foreground',
    info: 'bg-info-muted text-info-muted-foreground',
    success: 'bg-success-muted text-success',
    warning: 'bg-warning-muted text-warning',
    destructive: 'bg-destructive-muted text-destructive',
    destructiveSoft: 'bg-destructive-muted/70 text-destructive',
    primary: 'bg-primary-muted text-primary-muted-foreground',
  }

  const attentionNumber: Record<MetricTone, string> = {
    neutral: 'text-strong-foreground',
    info: 'text-strong-foreground',
    success: 'text-strong-foreground',
    warning: 'text-warning',
    destructive: 'text-destructive',
    destructiveSoft: 'text-destructive',
    primary: 'text-strong-foreground',
  }

  const numberClasses = $derived(
    isZero
      ? 'text-muted-foreground'
      : attention
        ? attentionNumber[tone]
        : 'text-strong-foreground',
  )
</script>

<button
  type="button"
  class={cn(
    'group flex w-full flex-col gap-3 rounded-lg border bg-surface p-4 text-left transition-colors disabled:opacity-100',
    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40',
    filter ? 'cursor-pointer hover:border-border-strong hover:bg-surface-hover' : 'cursor-default',
    active ? 'border-primary bg-primary-muted/40 ring-1 ring-primary' : 'border-border',
  )}
  aria-pressed={filter ? active : undefined}
  disabled={filter ? undefined : true}
  onclick={filter ? () => onFilter?.(filter) : undefined}
>
  <div class="flex items-center justify-between gap-2">
    <span class="truncate text-sm font-medium text-muted-foreground">{label}</span>
    <span class={cn('flex size-8 shrink-0 items-center justify-center rounded-md', chipClasses[tone])}>
      <Icon class="size-4" />
    </span>
  </div>
  <div class={cn('text-2xl font-bold tabular-nums transition-colors', numberClasses)}>
    {value}
  </div>
</button>
