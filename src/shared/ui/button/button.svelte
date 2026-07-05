<script module lang="ts">
  import { tv } from 'tailwind-variants'

  export const buttonVariants = tv({
    base: "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40 disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0",
    variants: {
      variant: {
        default: 'bg-primary text-primary-foreground shadow-xs hover:bg-primary/90',
        destructive:
          'bg-destructive text-destructive-foreground shadow-xs hover:bg-destructive/90',
        outline:
          'border border-input bg-background hover:bg-accent hover:text-accent-foreground',
        secondary:
          'bg-secondary text-secondary-foreground shadow-xs hover:bg-secondary/80',
        ghost: 'hover:bg-accent hover:text-accent-foreground',
        link: 'text-primary underline-offset-4 hover:underline',
      },
      size: {
        default: 'h-9 px-4 py-2',
        sm: 'h-8 rounded-md px-3 text-xs',
        lg: 'h-10 rounded-md px-8',
        icon: 'h-9 w-9',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  })
</script>

<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements'
  import type { Snippet } from 'svelte'
  import type { VariantProps } from 'tailwind-variants'
  import { Loader2 } from '@lucide/svelte'
  import { cn } from '@/shared/lib/utils'

  type ButtonVariant = VariantProps<typeof buttonVariants>['variant']
  type ButtonSize = VariantProps<typeof buttonVariants>['size']

  interface Props extends HTMLButtonAttributes {
    variant?: ButtonVariant
    size?: ButtonSize
    loading?: boolean
    icon?: Snippet
    children?: Snippet
    class?: string
  }

  let {
    class: className,
    variant = 'default',
    size = 'default',
    loading = false,
    icon,
    children,
    disabled,
    ...restProps
  }: Props = $props()
</script>

<button
  class={cn(buttonVariants({ variant, size }), className)}
  disabled={disabled || loading}
  {...restProps}
>
  {#if loading}
    <Loader2 class="size-4 animate-spin" />
  {:else if icon}
    {@render icon()}
  {/if}
  {#if children}{@render children()}{/if}
</button>
