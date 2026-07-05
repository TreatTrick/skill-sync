import { FolderOpen } from 'lucide-react'
import { useId } from 'react'

import { chooseDirectory } from '@/shared/lib'
import { t } from '@/shared/i18n'
import { cn } from '@/shared/lib/utils'

import { Button } from './button'
import { Input } from './input'

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
    <div className={cn('grid gap-1.5', className)}>
      <label
        className="text-sm font-medium text-muted-foreground"
        htmlFor={inputId}
      >
        {placeholder}
      </label>
      <div className="flex items-center gap-2">
        <Input
          id={inputId}
          onChange={(event) => onChange(event.target.value)}
          placeholder={placeholder}
          value={value}
        />
        <Button
          icon={<FolderOpen className="size-4" />}
          onClick={() => void handlePick()}
          type="button"
          variant="secondary"
        >
          {t('common.actions.browse')}
        </Button>
      </div>
    </div>
  )
}
