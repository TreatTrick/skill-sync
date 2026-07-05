import type { InputHTMLAttributes } from 'react'

import { cn } from '@/shared/lib/utils'

export const Input = ({
  className,
  ...props
}: InputHTMLAttributes<HTMLInputElement>) => (
  <input
    className={cn(
      'h-9 w-full rounded-lg border border-border bg-surface px-3 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/25',
      className,
    )}
    {...props}
  />
)
