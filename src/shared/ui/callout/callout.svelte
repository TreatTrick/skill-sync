<script module lang="ts">
  import { tv } from 'tailwind-variants'

  // Unified callout primitive (F2): tone x icon x rounded-md x spacing; replaces scattered hand-written banners
  export const calloutVariants = tv({
    base: 'flex items-start gap-2.5 rounded-md border p-3 text-sm',
    variants: {
      tone: {
        info: 'border-info-border bg-info-muted text-info-muted-foreground',
        warning: 'border-warning-border bg-warning-muted text-warning',
        danger:
          'border-destructive-border bg-destructive-muted text-destructive',
        success: 'border-success-border bg-success-muted text-success',
        brand:
          'border-primary-border bg-primary-muted text-primary-muted-foreground',
      },
    },
    defaultVariants: { tone: 'info' },
  })
</script>

<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { VariantProps } from 'tailwind-variants'
  import { cn } from '@/shared/lib/utils'

  type Tone = VariantProps<typeof calloutVariants>['tone']

  interface Props {
    tone?: Tone
    icon?: Snippet
    class?: string
    children: Snippet
  }

  let { tone = 'info', icon, class: className, children }: Props = $props()
</script>

<div class={cn(calloutVariants({ tone }), className)}>
  {#if icon}
    <span class="mt-0.5 shrink-0">{@render icon()}</span>
  {/if}
  <div class="min-w-0 flex-1">{@render children()}</div>
</div>
