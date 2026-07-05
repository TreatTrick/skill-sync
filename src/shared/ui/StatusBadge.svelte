<script lang="ts">
  import type { Snippet } from 'svelte'
  import { cn } from '@/shared/lib'

  type Tone = 'neutral' | 'success' | 'warning' | 'destructive' | 'info'

  interface Props {
    tone?: Tone
    class?: string
    children: Snippet
  }

  let { tone = 'neutral', class: className, children }: Props = $props()

  const toneClasses: Record<Tone, string> = {
    neutral: 'bg-surface-muted text-muted-foreground',
    success: 'bg-success-muted text-success',
    warning: 'bg-warning-muted text-warning',
    destructive: 'bg-destructive-muted text-destructive',
    info: 'bg-primary-muted text-primary-muted-foreground',
  }
</script>

<span
  class={cn(
    'inline-flex h-6 items-center rounded-full px-2.5 text-xs font-bold',
    toneClasses[tone],
    className,
  )}
>
  {@render children()}
</span>
