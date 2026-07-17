<script lang="ts">
  import type { Snippet } from 'svelte'
  import { cn } from '@/shared/lib/utils'

  interface Props {
    icon?: Snippet
    // Tint the icon circle; success gives positive end states a moment
    iconTone?: 'neutral' | 'success'
    title: string
    description?: string
    action?: Snippet
    class?: string
  }

  let {
    icon,
    iconTone = 'neutral',
    title,
    description,
    action,
    class: className,
  }: Props = $props()

  const circleClasses: Record<'neutral' | 'success', string> = {
    neutral: 'bg-surface-muted text-muted-foreground',
    success: 'bg-success-muted text-success',
  }
</script>

<div class={cn('grid gap-3 p-10 text-center', className)}>
  {#if icon}
    <div class={cn('mx-auto flex size-14 items-center justify-center rounded-full', circleClasses[iconTone])}>{@render icon()}</div>
  {/if}
  <div class="text-base font-semibold text-strong-foreground">{title}</div>
  {#if description}
    <p class="mx-auto max-w-md text-sm text-muted-foreground">{description}</p>
  {/if}
  {#if action}
    <div class="mt-2 flex justify-center">{@render action()}</div>
  {/if}
</div>
