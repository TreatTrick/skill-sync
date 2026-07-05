<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements'
  import { cn } from '@/shared/lib'

  interface Props extends HTMLInputAttributes {
    class?: string
  }

  let { class: className, ...rest }: Props = $props()
</script>

<input class={cn('size-4 rounded border-border accent-primary', className)} type="checkbox" {...rest} />
