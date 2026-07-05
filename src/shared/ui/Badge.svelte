<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import { cn } from '@/shared/lib'

  type Variant = 'default' | 'success' | 'warning' | 'destructive'

  interface Props extends HTMLAttributes<HTMLElement> {
    variant?: Variant
    class?: string
  }

  let { variant = 'default', class: className, ...rest }: Props = $props()

  const badgeVariants: Record<Variant, string> = {
    default: 'bg-primary-muted text-primary-muted-foreground',
    success: 'bg-success-muted text-success',
    warning: 'bg-warning-muted text-warning',
    destructive: 'bg-destructive-muted text-destructive',
  }
</script>

<span
  class={cn(
    'inline-flex h-6 items-center rounded-full px-2.5 text-xs font-bold',
    badgeVariants[variant],
    className,
  )}
  {...rest}
></span>
