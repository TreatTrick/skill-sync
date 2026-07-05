<script lang="ts">
  import type { Snippet } from 'svelte'
  import { cn } from '@/shared/lib'

  interface Props {
    title: string
    description?: string
    action?: Snippet
    class?: string
  }

  let { title, description, action, class: className }: Props = $props()
</script>

<div
  class={cn(
    'flex flex-col gap-3 border-b border-border p-4 sm:flex-row sm:items-center sm:justify-between',
    className,
  )}
>
  <div class="min-w-0">
    <h2 class="text-lg font-bold text-strong-foreground">{title}</h2>
    {#if description}
      <p class="mt-1 text-sm text-muted-foreground">{description}</p>
    {/if}
  </div>
  {#if action}
    <div class="shrink-0">{@render action()}</div>
  {/if}
</div>
