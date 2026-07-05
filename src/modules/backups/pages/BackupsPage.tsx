import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { RotateCcw } from 'lucide-react'
import { useState } from 'react'

import { errorMessage } from '@/shared/lib'
import { t } from '@/shared/i18n'

import { listBackups, restoreBackup } from '../api/backupsApi'

const formatSize = (bytes: number) => {
  if (bytes >= 1024 * 1024) {
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  }
  if (bytes >= 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`
  }
  return `${bytes} B`
}

const formatTime = (iso: string) => {
  if (!iso) {
    return '—'
  }
  const date = new Date(iso)
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString()
}

export const BackupsPage = () => {
  const queryClient = useQueryClient()
  const list = useQuery({ queryKey: ['backups'], queryFn: listBackups })
  const [msg, setMsg] = useState('')

  const restore = useMutation({
    mutationFn: (entry: { id: string; path: string }) =>
      restoreBackup(entry.id, entry.path),
    onSuccess: () => {
      setMsg(t('backups.restored'))
      void queryClient.invalidateQueries({ queryKey: ['backups'] })
    },
    onError: (error) =>
      setMsg(t('backups.restoreError', { message: errorMessage(error) })),
  })

  const backups = list.data ?? []

  return (
    <section className="grid gap-4">
      <div className="rounded-lg border border-border bg-surface p-4">
        <h2 className="text-lg font-bold text-strong-foreground">
          {t('backups.title')}
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">
          {t('backups.description')}
        </p>
      </div>

      {msg ? (
        <p className="rounded-lg border border-border bg-surface-muted p-3 text-sm text-foreground">
          {msg}
        </p>
      ) : null}

      {backups.length === 0 && !list.isLoading ? (
        <p className="rounded-lg border border-border bg-surface-muted p-4 text-sm text-muted-foreground">
          {t('backups.empty')}
        </p>
      ) : null}

      <div className="grid grid-cols-1 gap-2 lg:grid-cols-2">
        {backups.map((entry) => (
          <div
            className="grid gap-1 rounded-lg border border-border bg-surface p-3 text-sm"
            key={entry.id}
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="font-bold text-strong-foreground">
                {entry.skill_id}
              </span>
              <button
                className="inline-flex h-8 items-center gap-2 rounded-lg border border-border px-2.5 text-xs font-medium text-foreground hover:bg-surface-hover"
                disabled={restore.isPending}
                onClick={() =>
                  restore.mutate({ id: entry.id, path: entry.original_path })
                }
                type="button"
              >
                <RotateCcw className="size-3.5" />
                {t('backups.columns.actions')}
              </button>
            </div>
            <div className="grid grid-cols-1 gap-1 text-xs text-muted-foreground sm:grid-cols-2">
              <div className="truncate">
                {t('backups.columns.time')}: {formatTime(entry.created_at)}
              </div>
              <div className="truncate">
                {t('backups.columns.size')}: {formatSize(entry.size)}
              </div>
              <div className="truncate sm:col-span-2">
                {t('backups.columns.path')}: {entry.original_path}
              </div>
            </div>
          </div>
        ))}
      </div>
    </section>
  )
}
