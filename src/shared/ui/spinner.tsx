import { Loader2 } from 'lucide-react'

import { cn } from '@/shared/lib/utils'

export const Spinner = ({ className }: { className?: string }) => (
  <Loader2
    className={cn('size-4 animate-spin text-muted-foreground', className)}
  />
)
