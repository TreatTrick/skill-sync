import { FolderOpen } from 'lucide-react'
import { useId } from 'react'

import { chooseDirectory } from '@/shared/lib'
import { t } from '@/shared/i18n'
import { cn } from '@/shared/lib/utils'

interface PathPickerProps {
  value: string
  onChange: (path: string) => void
  placeholder: string
  className?: string
}

export const PathPicker = ({
  value,
  onChange,
  placeholder,
  className,
}: PathPickerProps) => {
  const inputId = useId()
  const handlePick = async () => {
    const path = await chooseDirectory()
    if (path) {
      onChange(path)
    }
  }

  return (
    <div className={cn('flex flex-col gap-1.5 sm:flex-row', className)}>
      <label
        className="text-sm font-medium text-muted-foreground"
        htmlFor={inputId}
      >
        {placeholder}
      </label>
      <div className="flex flex-1 items-center gap-2">
        <input
          className="h-9 flex-1 rounded-lg border border-border bg-surface px-3 text-sm text-foreground"
          id={inputId}
          onChange={(event) => onChange(event.target.value)}
          placeholder={placeholder}
          type="text"
          value={value}
        />
        <button
          className="inline-flex h-9 shrink-0 items-center gap-2 rounded-lg border border-border bg-surface px-3 text-sm font-medium text-foreground hover:bg-surface-hover"
          onClick={() => void handlePick()}
          type="button"
        >
          <FolderOpen className="size-4" />
          {t('common.actions.browse')}
        </button>
      </div>
    </div>
  )
}
