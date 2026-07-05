import { cn } from '@/shared/lib/utils'

interface StatusBadgeProps {
  tone?: 'neutral' | 'success' | 'warning' | 'destructive' | 'info'
  className?: string
  children: React.ReactNode
}

const toneClasses: Record<NonNullable<StatusBadgeProps['tone']>, string> = {
  neutral: 'bg-surface-muted text-muted-foreground',
  success: 'bg-success-muted text-success',
  warning: 'bg-warning-muted text-warning',
  destructive: 'bg-destructive-muted text-destructive',
  info: 'bg-primary-muted text-primary-muted-foreground',
}

export const StatusBadge = ({
  tone = 'neutral',
  className,
  children,
}: StatusBadgeProps) => (
  <span
    className={cn(
      'inline-flex h-6 items-center rounded-full px-2.5 text-xs font-bold',
      toneClasses[tone],
      className,
    )}
  >
    {children}
  </span>
)
