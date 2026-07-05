import { useQuery } from '@tanstack/react-query'
import { FolderOpen, Package, RefreshCw } from 'lucide-react'

import { errorMessage, openPath } from '@/shared/lib'
import { hostLabel, t } from '@/shared/i18n'
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardHeader,
  EmptyState,
  Spinner,
  StatusBadge,
} from '@/shared/ui'

import { scanSkills } from '../api/scanSkills'

const shortHash = (hash: string) =>
  hash.length > 12 ? hash.slice(0, 12) : hash

export const SkillsPage = () => {
  const scan = useQuery({ queryKey: ['scan-skills'], queryFn: scanSkills })
  const skills = scan.data?.skills ?? []
  const warnings = scan.data?.warnings ?? []

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader
          action={
            <Button
              icon={<RefreshCw className="size-4" />}
              loading={scan.isFetching}
              onClick={() => void scan.refetch()}
              variant="secondary"
            >
              {t('skills.rescan')}
            </Button>
          }
          description={t('skills.description')}
          title={t('skills.title')}
        />
      </Card>

      {scan.isLoading ? (
        <div className="flex justify-center py-12">
          <Spinner className="size-6" />
        </div>
      ) : null}

      {scan.error ? (
        <Card>
          <CardBody className="text-sm text-destructive">
            {errorMessage(scan.error)}
          </CardBody>
        </Card>
      ) : null}

      {warnings.length > 0 ? (
        <Card className="border-warning-border bg-warning-muted">
          <CardBody className="text-sm text-warning">
            <div className="font-bold">{t('skills.warnings')}</div>
            <ul className="mt-1 grid gap-1">
              {warnings.map((warning, index) => (
                <li key={index}>{warning}</li>
              ))}
            </ul>
          </CardBody>
        </Card>
      ) : null}

      {skills.length === 0 && !scan.isLoading ? (
        <Card>
          <EmptyState
            icon={<Package className="size-10" />}
            title={t('skills.empty')}
          />
        </Card>
      ) : null}

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
        {skills.map((skill) => (
          <Card className="p-4" key={skill.id}>
            <div className="flex flex-wrap items-start justify-between gap-2">
              <div className="grid gap-1">
                <div className="text-base font-bold text-strong-foreground">
                  {skill.name}
                </div>
                <Badge variant="default">{hostLabel(skill.host)}</Badge>
              </div>
              <StatusBadge tone="success">{t('skills.enabled')}</StatusBadge>
            </div>
            <p className="mt-2 text-sm text-muted-foreground">
              {skill.description}
            </p>
            <div className="mt-3 grid grid-cols-1 gap-1 text-xs text-muted-foreground sm:grid-cols-2">
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
            <div className="mt-3 flex justify-end">
              <Button
                icon={<FolderOpen className="size-3.5" />}
                onClick={() => void openPath(skill.source_path)}
                size="sm"
                variant="secondary"
              >
                {t('skills.openFolder')}
              </Button>
            </div>
          </Card>
        ))}
      </div>
    </div>
  )
}
