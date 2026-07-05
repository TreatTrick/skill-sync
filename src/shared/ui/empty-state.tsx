import type { ReactNode } from 'react'

import { cn } from '@/shared/lib/utils'

interface EmptyStateProps {
  icon?: ReactNode
  title: string
  description?: string
  action?: ReactNode
  className?: string
}

export const EmptyState = ({
  icon,
  title,
  description,
  action,
  className,
}: EmptyStateProps) => (
  <div className={cn('grid gap-2 p-8 text-center', className)}>
    {icon ? <div className="mx-auto text-muted-foreground">{icon}</div> : null}
    <div className="text-sm font-bold text-strong-foreground">{title}</div>
    {description ? (
      <p className="mx-auto max-w-md text-sm text-muted-foreground">
        {description}
      </p>
    ) : null}
    {action ? <div className="mt-2 flex justify-center">{action}</div> : null}
  </div>
)
