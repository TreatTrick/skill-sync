<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements'
  import { cn } from '@/shared/lib'

  interface Props extends HTMLInputAttributes {
    class?: string
  }

  let { class: className, value = $bindable(), ...rest }: Props = $props()
</script>

<input
  bind:value
  class={cn(
    'h-9 w-full rounded-lg border border-border bg-surface px-3 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/25',
    className,
  )}
  {...rest}
/>
