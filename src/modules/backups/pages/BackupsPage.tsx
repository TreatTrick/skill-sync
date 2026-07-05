import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { RotateCcw, Save } from 'lucide-react'
import { useState } from 'react'

import { errorMessage } from '@/shared/lib'
import { t } from '@/shared/i18n'
import {
  Button,
  Card,
  CardBody,
  CardHeader,
  EmptyState,
  Spinner,
} from '@/shared/ui'

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
    <div className="grid gap-4">
      <Card>
        <CardHeader
          description={t('backups.description')}
          title={t('backups.title')}
        />
      </Card>

      {msg ? (
        <Card className="border-success-muted bg-success-muted">
          <CardBody className="flex items-center gap-2 text-sm text-success">
            <Save className="size-4 shrink-0" />
            {msg}
          </CardBody>
        </Card>
      ) : null}

      {list.isLoading ? (
        <div className="flex justify-center py-12">
          <Spinner className="size-6" />
        </div>
      ) : null}

      {backups.length === 0 && !list.isLoading ? (
        <Card>
          <EmptyState
            icon={<RotateCcw className="size-10" />}
            title={t('backups.empty')}
          />
        </Card>
      ) : null}

      <div className="grid grid-cols-1 gap-2 lg:grid-cols-2">
        {backups.map((entry) => (
          <Card className="p-3" key={entry.id}>
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span className="font-bold text-strong-foreground">
                {entry.skill_id}
              </span>
              <Button
                disabled={restore.isPending}
                icon={<RotateCcw className="size-3.5" />}
                onClick={() =>
                  restore.mutate({ id: entry.id, path: entry.original_path })
                }
                size="sm"
                variant="secondary"
              >
                {t('backups.columns.actions')}
              </Button>
            </div>
            <div className="mt-2 grid grid-cols-1 gap-1 text-xs text-muted-foreground sm:grid-cols-2">
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
          </Card>
        ))}
      </div>
    </div>
  )
}
