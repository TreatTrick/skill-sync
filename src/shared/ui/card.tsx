import type { ReactNode } from 'react'

import { cn } from '@/shared/lib/utils'

interface CardProps {
  children: ReactNode
  className?: string
}

export const Card = ({ children, className }: CardProps) => (
  <div
    className={cn(
      'rounded-xl border border-border bg-surface shadow-sm',
      className,
    )}
  >
    {children}
  </div>
)

interface CardHeaderProps {
  title: string
  description?: string
  action?: ReactNode
  className?: string
}

export const CardHeader = ({
  title,
  description,
  action,
  className,
}: CardHeaderProps) => (
  <div
    className={cn(
      'flex flex-col gap-3 border-b border-border p-4 sm:flex-row sm:items-center sm:justify-between',
      className,
    )}
  >
    <div className="min-w-0">
      <h2 className="text-lg font-bold text-strong-foreground">{title}</h2>
      {description ? (
        <p className="mt-1 text-sm text-muted-foreground">{description}</p>
      ) : null}
    </div>
    {action ? <div className="shrink-0">{action}</div> : null}
  </div>
)

interface CardBodyProps {
  children: ReactNode
  className?: string
}

export const CardBody = ({ children, className }: CardBodyProps) => (
  <div className={cn('p-4', className)}>{children}</div>
)
