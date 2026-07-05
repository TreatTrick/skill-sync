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

<p class={cn('text-sm text-muted-foreground', className)} {...restProps}>
  {@render children()}
</p>
