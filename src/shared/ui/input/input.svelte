<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements'
  import { cn } from '@/shared/lib/utils'

  let {
    class: className,
    value = $bindable(),
    ...restProps
  }: HTMLInputAttributes = $props()
</script>

<input
  bind:value
  class={cn(
    'flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40 disabled:cursor-not-allowed disabled:opacity-50',
    className,
  )}
  {...restProps}
/>
