<script lang="ts">
  import type { HTMLTextareaAttributes } from 'svelte/elements'
  import { cn } from '@/shared/lib'

  interface Props extends HTMLTextareaAttributes {
    class?: string
  }

  let { class: className, value = $bindable(), ...rest }: Props = $props()
</script>

<textarea
  bind:value
  class={cn(
    'min-h-24 w-full rounded-lg border border-border bg-surface p-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/25',
    className,
  )}
  {...rest}
></textarea>
