<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import type { Snippet } from 'svelte'
  import { cn } from '@/shared/lib/utils'

  let {
    class: className,
    children,
    ...restProps
  }: HTMLAttributes<HTMLElement> & { children: Snippet } = $props()
</script>

<div class={cn('flex flex-col space-y-1.5 p-6', className)} {...restProps}>
  {@render children()}
</div>
