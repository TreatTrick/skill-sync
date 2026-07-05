import type { InputHTMLAttributes } from 'react'

import { cn } from '@/shared/lib/utils'

export const Checkbox = ({
  className,
  ...props
}: InputHTMLAttributes<HTMLInputElement>) => (
  <input
    className={cn('size-4 rounded border-border accent-primary', className)}
    type="checkbox"
    {...props}
  />
)
