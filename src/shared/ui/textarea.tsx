import type { TextareaHTMLAttributes } from 'react'

import { cn } from '@/shared/lib/utils'

export const Textarea = ({
  className,
  ...props
}: TextareaHTMLAttributes<HTMLTextAreaElement>) => (
  <textarea
    className={cn(
      'min-h-24 w-full rounded-lg border border-border bg-surface p-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/25',
      className,
    )}
    {...props}
  />
)
