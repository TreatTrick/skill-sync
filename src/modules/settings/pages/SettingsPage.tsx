import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import { errorMessage } from '@/shared/lib'
import { t } from '@/shared/i18n'
import {
  Button,
  Card,
  CardBody,
  CardHeader,
  Checkbox,
  Input,
  PathPicker,
  Spinner,
  Textarea,
} from '@/shared/ui'

import { getAppState, saveConfig } from '../api/configApi'
import type { AppConfig } from '../schemas/config'

const toLines = (paths: string[]) => paths.join('\n')
const fromLines = (text: string) =>
  text
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0)

export const SettingsPage = () => {
  const queryClient = useQueryClient()
  const state = useQuery({ queryKey: ['app-state'], queryFn: getAppState })
  const [config, setConfig] = useState<AppConfig | null>(null)
  const [codexPaths, setCodexPaths] = useState('')
  const [claudePaths, setClaudePaths] = useState('')
  const [ignore, setIgnore] = useState('')
  const [msg, setMsg] = useState('')
  const [prefilled, setPrefilled] = useState(false)

  if (!prefilled && state.data) {
    setPrefilled(true)
    setConfig(state.data.config)
    setCodexPaths(toLines(state.data.config.hosts.codex.paths))
    setClaudePaths(toLines(state.data.config.hosts.claude.paths))
    setIgnore(toLines(state.data.config.ignore))
  }

  const save = useMutation({
    mutationFn: (cfg: AppConfig) => saveConfig(cfg),
    onSuccess: () => {
      setMsg(t('settings.saved'))
      void queryClient.invalidateQueries({ queryKey: ['app-state'] })
    },
    onError: (error) =>
      setMsg(t('settings.saveError', { message: errorMessage(error) })),
  })

  const updateRepo = (patch: Partial<AppConfig['repository']>) =>
    setConfig((current) =>
      current
        ? { ...current, repository: { ...current.repository, ...patch } }
        : current,
    )

  const updateHost = (
    host: 'codex' | 'claude',
    patch: Partial<AppConfig['hosts']['codex']>,
  ) =>
    setConfig((current) =>
      current
        ? {
            ...current,
            hosts: {
              ...current.hosts,
              [host]: { ...current.hosts[host], ...patch },
            },
          }
        : current,
    )

  const updateDefaults = (patch: Partial<AppConfig['defaults']>) =>
    setConfig((current) =>
      current
        ? { ...current, defaults: { ...current.defaults, ...patch } }
        : current,
    )

  const handleSave = () => {
    if (!config) {
      return
    }
    setMsg('')
    save.mutate({
      ...config,
      hosts: {
        codex: { ...config.hosts.codex, paths: fromLines(codexPaths) },
        claude: { ...config.hosts.claude, paths: fromLines(claudePaths) },
      },
      ignore: fromLines(ignore),
    })
  }

  return (
    <div className="grid gap-4">
      <Card>
        <CardHeader
          action={
            <Button
              disabled={!config}
              loading={save.isPending}
              onClick={handleSave}
            >
              {t('settings.save')}
            </Button>
          }
          description={t('settings.description')}
          title={t('settings.title')}
        />
      </Card>

      {msg ? (
        <Card className="border-success-muted bg-success-muted">
          <CardBody className="text-sm text-success">{msg}</CardBody>
        </Card>
      ) : null}

      {state.error ? (
        <Card className="border-destructive-border bg-destructive-muted">
          <CardBody className="text-sm text-destructive">
            {errorMessage(state.error)}
          </CardBody>
        </Card>
      ) : null}

      {!config ? (
        <div className="flex justify-center py-12">
          <Spinner className="size-6" />
        </div>
      ) : (
        <>
          <Card>
            <CardHeader title={t('settings.repository')} />
            <CardBody className="grid gap-4">
              <PathPicker
                onChange={(path) => updateRepo({ local_path: path })}
                placeholder={t('settings.localPath')}
                value={config.repository.local_path}
              />
              <label className="grid gap-1.5 text-sm font-medium text-muted-foreground">
                {t('settings.remote')}
                <Input
                  onChange={(event) =>
                    updateRepo({ remote: event.target.value })
                  }
                  value={config.repository.remote}
                />
              </label>
              <label className="grid gap-1.5 text-sm font-medium text-muted-foreground">
                {t('settings.branch')}
                <Input
                  onChange={(event) =>
                    updateRepo({ branch: event.target.value })
                  }
                  value={config.repository.branch}
                />
              </label>
            </CardBody>
          </Card>

          <Card>
            <CardHeader title={t('settings.hosts')} />
            <CardBody className="grid gap-4">
              <label className="flex items-center gap-2 text-sm text-foreground">
                <Checkbox
                  checked={config.hosts.codex.enabled}
                  onChange={(event) =>
                    updateHost('codex', { enabled: event.target.checked })
                  }
                />
                {t('settings.codexEnabled')}
              </label>
              <label className="grid gap-1.5 text-sm font-medium text-muted-foreground">
                {t('settings.codexPaths')}
                <Textarea
                  onChange={(event) => setCodexPaths(event.target.value)}
                  value={codexPaths}
                />
              </label>
              <label className="flex items-center gap-2 text-sm text-foreground">
                <Checkbox
                  checked={config.hosts.claude.enabled}
                  onChange={(event) =>
                    updateHost('claude', { enabled: event.target.checked })
                  }
                />
                {t('settings.claudeEnabled')}
              </label>
              <label className="grid gap-1.5 text-sm font-medium text-muted-foreground">
                {t('settings.claudePaths')}
                <Textarea
                  onChange={(event) => setClaudePaths(event.target.value)}
                  value={claudePaths}
                />
              </label>
            </CardBody>
          </Card>

          <Card>
            <CardBody className="grid gap-4">
              <label className="flex items-center gap-2 text-sm text-foreground">
                <Checkbox
                  checked={config.defaults.backup}
                  onChange={(event) =>
                    updateDefaults({ backup: event.target.checked })
                  }
                />
                {t('settings.backup')}
              </label>
              <label className="grid gap-1.5 text-sm font-medium text-muted-foreground">
                {t('settings.ignore')}
                <Textarea
                  onChange={(event) => setIgnore(event.target.value)}
                  value={ignore}
                />
              </label>
            </CardBody>
          </Card>
        </>
      )}
    </div>
  )
}
