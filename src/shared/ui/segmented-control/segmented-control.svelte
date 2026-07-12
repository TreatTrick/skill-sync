<script lang="ts">
  import type { Component } from 'svelte'
  import { cn } from '@/shared/lib/utils'

  interface Option {
    value: string
    label: string
    icon?: Component<{ class?: string }>
  }

  interface Props {
    options: Option[]
    value: string
    onSelect: (value: string) => void
    class?: string
  }

  let { options, value, onSelect, class: className }: Props = $props()
</script>

<div class={cn('flex flex-wrap gap-2', className)}>
  {#each options as option (option.value)}
    {@const Icon = option.icon}
    <button
      class={cn(
        'flex h-9 flex-1 items-center justify-center gap-1.5 rounded-md border text-sm font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/40',
        value === option.value
          ? 'border-primary bg-primary-muted text-primary-muted-foreground'
          : 'border-border bg-surface text-foreground hover:bg-surface-hover',
      )}
      onclick={() => onSelect(option.value)}
      type="button"
    >
      {#if Icon}<Icon class="size-4" />{/if}
      {option.label}
    </button>
  {/each}
</div>
