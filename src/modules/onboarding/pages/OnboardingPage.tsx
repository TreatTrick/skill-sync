import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'

import { errorMessage } from '@/shared/lib'
import { t } from '@/shared/i18n'
import { PathPicker } from '@/shared/ui'
import { getAppState } from '@/modules/settings'

import { checkGit, checkRemote, prepareRepo } from '../api/onboardingApi'
import type { GitCheck, RemoteCheck } from '../schemas/onboarding'

export const OnboardingPage = () => {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const state = useQuery({ queryKey: ['app-state'], queryFn: getAppState })
  const [localPath, setLocalPath] = useState('')
  const [remote, setRemote] = useState('')
  const [branch, setBranch] = useState('main')
  const [gitCheck, setGitCheck] = useState<GitCheck | null>(null)
  const [remoteCheck, setRemoteCheck] = useState<RemoteCheck | null>(null)
  const [msg, setMsg] = useState('')
  const [saving, setSaving] = useState(false)
  const [prefilled, setPrefilled] = useState(false)

  if (!prefilled && state.data) {
    setPrefilled(true)
    setLocalPath(state.data.config.repository.local_path)
    setRemote(state.data.config.repository.remote)
    setBranch(state.data.config.repository.branch || 'main')
  }

  const handleCheckGit = async () => {
    try {
      setGitCheck(await checkGit())
    } catch (error) {
      setMsg(errorMessage(error))
    }
  }

  const handleCheckRemote = async () => {
    try {
      setRemoteCheck(await checkRemote(remote))
    } catch (error) {
      setMsg(errorMessage(error))
    }
  }

  const handleSave = async () => {
    if (gitCheck && !gitCheck.available) {
      setMsg(t('onboarding.needGit'))
      return
    }
    if (!localPath.trim()) {
      setMsg(t('onboarding.needGit'))
      return
    }
    setSaving(true)
    setMsg('')
    try {
      await prepareRepo(localPath, remote, branch)
      await queryClient.invalidateQueries({ queryKey: ['app-state'] })
      navigate('/app/dashboard')
    } catch (error) {
      setMsg(errorMessage(error))
    } finally {
      setSaving(false)
    }
  }

  return (
    <section className="grid gap-4">
      <div className="rounded-lg border border-border bg-surface p-4">
        <h2 className="text-lg font-bold text-strong-foreground">
          {t('onboarding.title')}
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">
          {t('onboarding.description')}
        </p>
      </div>

      {msg ? (
        <p className="rounded-lg border border-destructive-border bg-destructive-muted p-3 text-sm text-destructive">
          {msg}
        </p>
      ) : null}

      <div className="grid gap-3 rounded-lg border border-border bg-surface p-4">
        <PathPicker
          onChange={setLocalPath}
          placeholder={t('onboarding.localPath')}
          value={localPath}
        />
        <label className="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('onboarding.remote')}
          <input
            className="h-9 rounded-lg border border-border bg-surface px-3 text-sm text-foreground"
            onChange={(event) => setRemote(event.target.value)}
            type="text"
            value={remote}
          />
        </label>
        <label className="grid gap-1.5 text-sm font-medium text-muted-foreground">
          {t('onboarding.branch')}
          <input
            className="h-9 rounded-lg border border-border bg-surface px-3 text-sm text-foreground"
            onChange={(event) => setBranch(event.target.value)}
            type="text"
            value={branch}
          />
        </label>
      </div>

      <div className="grid gap-3 rounded-lg border border-border bg-surface p-4 sm:grid-cols-2">
        <div className="grid gap-2">
          <div className="flex items-center justify-between gap-2">
            <span className="text-sm font-bold text-strong-foreground">
              {t('onboarding.gitCheck')}
            </span>
            <button
              className="inline-flex h-8 items-center rounded-lg border border-border px-2.5 text-xs font-medium text-foreground hover:bg-surface-hover"
              onClick={() => void handleCheckGit()}
              type="button"
            >
              {t('onboarding.checkGit')}
            </button>
          </div>
          <p className="text-xs text-muted-foreground">
            {gitCheck
              ? gitCheck.available
                ? t('onboarding.gitOk', { version: gitCheck.version })
                : t('onboarding.gitMissing')
              : '—'}
          </p>
        </div>
        <div className="grid gap-2">
          <div className="flex items-center justify-between gap-2">
            <span className="text-sm font-bold text-strong-foreground">
              {t('onboarding.remoteCheck')}
            </span>
            <button
              className="inline-flex h-8 items-center rounded-lg border border-border px-2.5 text-xs font-medium text-foreground hover:bg-surface-hover"
              disabled={!remote.trim()}
              onClick={() => void handleCheckRemote()}
              type="button"
            >
              {t('onboarding.checkRemote')}
            </button>
          </div>
          <p className="text-xs text-muted-foreground">
            {remoteCheck
              ? remoteCheck.ok
                ? t('onboarding.remoteOk')
                : t('onboarding.remoteFail', { message: remoteCheck.message })
              : '—'}
          </p>
        </div>
      </div>

      <button
        className="inline-flex h-9 items-center justify-center gap-2 rounded-lg bg-primary px-3 text-sm font-bold text-primary-foreground"
        disabled={saving}
        onClick={() => void handleSave()}
        type="button"
      >
        {saving ? t('common.status.loading') : t('onboarding.save')}
      </button>
    </section>
  )
}
