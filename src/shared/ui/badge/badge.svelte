<script module lang="ts">
  import { tv } from 'tailwind-variants'

  export const badgeVariants = tv({
    base: 'inline-flex items-center rounded-md border px-2.5 py-0.5 text-xs font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40',
    variants: {
      variant: {
        default:
          'border-transparent bg-primary text-primary-foreground shadow hover:bg-primary/80',
        secondary:
          'border-transparent bg-secondary text-secondary-foreground hover:bg-secondary/80',
        destructive:
          'border-transparent bg-destructive text-destructive-foreground shadow hover:bg-destructive/80',
        outline: 'text-foreground',
      },
    },
    defaultVariants: {
      variant: 'default',
    },
  })
</script>

<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import type { VariantProps } from 'tailwind-variants'
  import { cn } from '@/shared/lib/utils'

  type BadgeVariant = VariantProps<typeof badgeVariants>['variant']

  interface Props extends HTMLAttributes<HTMLElement> {
    variant?: BadgeVariant
    class?: string
  }

  let { class: className, variant = 'default', ...restProps }: Props = $props()
</script>

<span class={cn(badgeVariants({ variant }), className)} {...restProps}></span>
