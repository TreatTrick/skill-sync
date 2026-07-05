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

<div class={cn('p-6 pt-0', className)} {...restProps}>
  {@render children()}
</div>
