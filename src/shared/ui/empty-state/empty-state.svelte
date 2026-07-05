<script lang="ts">
  import type { Snippet } from 'svelte'
  import { cn } from '@/shared/lib/utils'

  interface Props {
    icon?: Snippet
    title: string
    description?: string
    action?: Snippet
    class?: string
  }

  let { icon, title, description, action, class: className }: Props = $props()
</script>

<div class={cn('grid gap-2 p-8 text-center', className)}>
  {#if icon}
    <div class="mx-auto text-muted-foreground">{@render icon()}</div>
  {/if}
  <div class="text-sm font-bold text-strong-foreground">{title}</div>
  {#if description}
    <p class="mx-auto max-w-md text-sm text-muted-foreground">{description}</p>
  {/if}
  {#if action}
    <div class="mt-2 flex justify-center">{@render action()}</div>
  {/if}
</div>
