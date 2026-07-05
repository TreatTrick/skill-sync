import type { ComponentProps } from 'react'

import { cn } from '@/shared/lib/utils'

interface BadgeProps extends ComponentProps<'span'> {
  variant?: 'default' | 'success' | 'warning' | 'destructive'
}

const badgeVariants: Record<Required<BadgeProps>['variant'], string> = {
  default: 'bg-primary-muted text-primary-muted-foreground',
  success: 'bg-success-muted text-success',
  warning: 'bg-warning-muted text-warning',
  destructive: 'bg-destructive-muted text-destructive',
}

export const Badge = ({
  className,
  variant = 'default',
  ...props
}: BadgeProps) => (
  <span
    className={cn(
      'inline-flex h-6 items-center rounded-full px-2.5 text-xs font-bold',
      badgeVariants[variant],
      className,
    )}
    {...props}
  />
)
