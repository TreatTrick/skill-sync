<script lang="ts">
  import type { Snippet } from 'svelte'
  import { cn } from '@/shared/lib'

  interface Props {
    children: Snippet
    class?: string
  }

  let { children, class: className }: Props = $props()
</script>

<div class={cn('p-4', className)}>{@render children()}</div>
