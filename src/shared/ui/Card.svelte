<script lang="ts">
  import type { Snippet } from 'svelte'
  import { cn } from '@/shared/lib'

  interface Props {
    children: Snippet
    class?: string
  }

  let { children, class: className }: Props = $props()
</script>

<div class={cn('rounded-xl border border-border bg-surface shadow-sm', className)}>
  {@render children()}
</div>
