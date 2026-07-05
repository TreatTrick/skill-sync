<script lang="ts">
  import type { HTMLTextareaAttributes } from 'svelte/elements'
  import { cn } from '@/shared/lib/utils'

  let {
    class: className,
    value = $bindable(),
    ...restProps
  }: HTMLTextareaAttributes = $props()
</script>

<textarea
  bind:value
  class={cn(
    'flex min-h-[60px] w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40 disabled:cursor-not-allowed disabled:opacity-50',
    className,
  )}
  {...restProps}
></textarea>
