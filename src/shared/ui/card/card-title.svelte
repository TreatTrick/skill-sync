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

<h3 class={cn('font-semibold leading-none tracking-tight', className)} {...restProps}>
  {@render children()}
</h3>
