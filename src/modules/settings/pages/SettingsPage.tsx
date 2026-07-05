import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'

import { errorMessage } from '@/shared/lib'
import { t } from '@/shared/i18n'
import { PathPicker } from '@/shared/ui'

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

  if (state.error) {
    return (
      <p className="rounded-lg border border-destructive-border bg-destructive-muted p-3 text-sm text-destructive">
        {errorMessage(state.error)}
      </p>
    )
  }

  if (!config) {
    return (
      <p className="text-sm text-muted-foreground">
        {t('common.status.loading')}
      </p>
    )
  }

  return (
    <section className="grid gap-4">
      <div className="flex flex-col justify-between gap-3 rounded-lg border border-border bg-surface p-4 sm:flex-row sm:items-center">
        <div>
          <h2 className="text-lg font-bold text-strong-foreground">
            {t('settings.title')}
          </h2>
          <p className="mt-1 text-sm text-muted-foreground">
            {t('settings.description')}
          </p>
        </div>
        <button
          className="inline-flex h-9 items-center justify-center gap-2 rounded-lg bg-primary px-3 text-sm font-bold text-primary-foreground"
          disabled={save.isPending}
          onClick={handleSave}
          type="button"
        >
          {t('settings.save')}
        </button>
      </div>

      {msg ? (
        <p className="rounded-lg border border-border bg-surface-muted p-3 text-sm text-foreground">
          {msg}
        </p>
      ) : null}

      <div className="grid gap-3 rounded-lg border border-border bg-surface p-4">
        <h3 className="text-sm font-bold text-strong-foreground">
          {t('settings.repository')}
        </h3>
        <PathPicker
          onChange={(path) => updateRepo({ local_path: path })}
          placeholder={t('settings.localPath')}
          value={config.repository.local_path}
        />
        <label className="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('settings.remote')}
          <input
            className="h-9 rounded-lg border border-border bg-surface px-3 text-sm text-foreground"
            onChange={(event) => updateRepo({ remote: event.target.value })}
            type="text"
            value={config.repository.remote}
          />
        </label>
        <label className="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('settings.branch')}
          <input
            className="h-9 rounded-lg border border-border bg-surface px-3 text-sm text-foreground"
            onChange={(event) => updateRepo({ branch: event.target.value })}
            type="text"
            value={config.repository.branch}
          />
        </label>
      </div>

      <div className="grid gap-3 rounded-lg border border-border bg-surface p-4">
        <h3 className="text-sm font-bold text-strong-foreground">
          {t('settings.hosts')}
        </h3>
        <label className="flex items-center gap-2 text-sm text-foreground">
          <input
            checked={config.hosts.codex.enabled}
            onChange={(event) =>
              updateHost('codex', { enabled: event.target.checked })
            }
            type="checkbox"
          />
          {t('settings.codexEnabled')}
        </label>
        <label className="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('settings.codexPaths')}
          <textarea
            className="min-h-24 rounded-lg border border-border bg-surface p-2 text-sm text-foreground"
            onChange={(event) => setCodexPaths(event.target.value)}
            value={codexPaths}
          />
        </label>
        <label className="flex items-center gap-2 text-sm text-foreground">
          <input
            checked={config.hosts.claude.enabled}
            onChange={(event) =>
              updateHost('claude', { enabled: event.target.checked })
            }
            type="checkbox"
          />
          {t('settings.claudeEnabled')}
        </label>
        <label className="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('settings.claudePaths')}
          <textarea
            className="min-h-24 rounded-lg border border-border bg-surface p-2 text-sm text-foreground"
            onChange={(event) => setClaudePaths(event.target.value)}
            value={claudePaths}
          />
        </label>
      </div>

      <div className="grid gap-3 rounded-lg border border-border bg-surface p-4">
        <label className="flex items-center gap-2 text-sm text-foreground">
          <input
            checked={config.defaults.backup}
            onChange={(event) =>
              updateDefaults({ backup: event.target.checked })
            }
            type="checkbox"
          />
          {t('settings.backup')}
        </label>
        <label className="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('settings.ignore')}
          <textarea
            className="min-h-24 rounded-lg border border-border bg-surface p-2 text-sm text-foreground"
            onChange={(event) => setIgnore(event.target.value)}
            value={ignore}
          />
        </label>
      </div>
    </section>
  )
}
