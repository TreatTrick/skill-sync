import { useQuery } from '@tanstack/react-query'
import { FolderOpen, RefreshCw } from 'lucide-react'

import { errorMessage, openPath } from '@/shared/lib'
import { hostLabel, t } from '@/shared/i18n'
import { Badge, StatusBadge } from '@/shared/ui'

import { scanSkills } from '../api/scanSkills'

const shortHash = (hash: string) =>
  hash.length > 12 ? hash.slice(0, 12) : hash

export const SkillsPage = () => {
  const scan = useQuery({ queryKey: ['scan-skills'], queryFn: scanSkills })

  const handleRescan = () => {
    void scan.refetch()
  }

  const handleOpen = (path: string) => {
    void openPath(path)
  }

  const skills = scan.data?.skills ?? []
  const warnings = scan.data?.warnings ?? []

  return (
    <section className="grid gap-4">
      <div className="flex flex-col justify-between gap-3 rounded-lg border border-border bg-surface p-4 sm:flex-row sm:items-center">
        <div>
          <h2 className="text-lg font-bold text-strong-foreground">
            {t('skills.title')}
          </h2>
          <p className="mt-1 text-sm text-muted-foreground">
            {t('skills.description')}
          </p>
        </div>
        <button
          className="inline-flex h-9 items-center justify-center gap-2 rounded-lg bg-primary px-3 text-sm font-bold text-primary-foreground"
          disabled={scan.isFetching}
          onClick={handleRescan}
          type="button"
        >
          <RefreshCw className="size-4" />
          {t('skills.rescan')}
        </button>
      </div>

      {scan.error ? (
        <p className="rounded-lg border border-destructive-border bg-destructive-muted p-3 text-sm text-destructive">
          {errorMessage(scan.error)}
        </p>
      ) : null}

      {warnings.length > 0 ? (
        <div className="rounded-lg border border-warning-border bg-warning-muted p-3 text-sm text-warning">
          <div className="font-bold">{t('skills.warnings')}</div>
          <ul className="mt-1 grid gap-1">
            {warnings.map((warning, index) => (
              <li key={index}>{warning}</li>
            ))}
          </ul>
        </div>
      ) : null}

      {skills.length === 0 && !scan.isLoading ? (
        <p className="rounded-lg border border-border bg-surface-muted p-4 text-sm text-muted-foreground">
          {t('skills.empty')}
        </p>
      ) : null}

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        {skills.map((skill) => (
          <div
            className="grid gap-2 rounded-lg border border-border bg-surface p-4"
            key={skill.id}
          >
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="text-base font-bold text-strong-foreground">
                {skill.name}
              </div>
              <Badge variant="default">{hostLabel(skill.host)}</Badge>
            </div>
            <p className="text-sm text-muted-foreground">{skill.description}</p>
            <div className="grid grid-cols-1 gap-1 text-xs text-muted-foreground sm:grid-cols-2">
              <div className="truncate">
                <span className="text-foreground">
                  {t('skills.columns.path')}:
                </span>{' '}
                {skill.source_path}
              </div>
              <div className="truncate">
                <span className="text-foreground">
                  {t('skills.columns.modified')}:
                </span>{' '}
                {skill.modified_at || '—'}
              </div>
              <div className="truncate sm:col-span-2">
                <span className="text-foreground">
                  {t('skills.columns.hash')}:
                </span>{' '}
                {shortHash(skill.hash)}
              </div>
            </div>
            <div className="flex items-center justify-between gap-2">
              <StatusBadge tone="success">{t('skills.enabled')}</StatusBadge>
              <button
                className="inline-flex h-8 items-center gap-2 rounded-lg border border-border px-2.5 text-xs font-medium text-foreground hover:bg-surface-hover"
                onClick={() => handleOpen(skill.source_path)}
                type="button"
              >
                <FolderOpen className="size-3.5" />
                {t('skills.openFolder')}
              </button>
            </div>
          </div>
        ))}
      </div>
    </section>
  )
}
