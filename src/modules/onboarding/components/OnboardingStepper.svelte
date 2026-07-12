<script lang="ts">
  import { cn } from '@/shared/lib'

  interface Props {
    current: number
    total?: number
    ariaLabel?: string
  }

  let { current, total = 5, ariaLabel }: Props = $props()

  const steps = $derived(Array.from({ length: total }, (_, i) => i))
</script>

<ol class="flex items-center gap-2" aria-label={ariaLabel}>
  {#each steps as i (i)}
    <li class="flex flex-1 items-center gap-2">
      <span
        class={cn(
          'flex size-6 shrink-0 items-center justify-center rounded-full text-xs font-semibold transition-colors',
          i < current - 1
            ? 'bg-primary text-primary-foreground'
            : i === current - 1
              ? 'bg-primary-muted text-primary-muted-foreground ring-2 ring-primary'
              : 'bg-surface-muted text-muted-foreground',
        )}
      >
        {i + 1}
      </span>
      {#if i < total - 1}
        <span
          class={cn('h-0.5 flex-1 rounded-full', i < current - 1 ? 'bg-primary' : 'bg-border')}
        ></span>
      {/if}
    </li>
  {/each}
</ol>
