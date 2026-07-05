import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useNavigate } from 'react-router-dom'

import { errorMessage } from '@/shared/lib'
import { t } from '@/shared/i18n'
import {
  Button,
  Card,
  CardBody,
  CardHeader,
  Input,
  PathPicker,
} from '@/shared/ui'
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
    <div className="grid max-w-2xl gap-4">
      <Card>
        <CardHeader
          description={t('onboarding.description')}
          title={t('onboarding.title')}
        />
      </Card>

      {msg ? (
        <Card className="border-destructive-border bg-destructive-muted">
          <CardBody className="text-sm text-destructive">{msg}</CardBody>
        </Card>
      ) : null}

      <Card>
        <CardBody className="grid gap-4">
          <PathPicker
            onChange={setLocalPath}
            placeholder={t('onboarding.localPath')}
            value={localPath}
          />
          <label className="grid gap-1.5 text-sm font-medium text-muted-foreground">
            {t('onboarding.remote')}
            <Input
              onChange={(event) => setRemote(event.target.value)}
              value={remote}
            />
          </label>
          <label className="grid gap-1.5 text-sm font-medium text-muted-foreground">
            {t('onboarding.branch')}
            <Input
              onChange={(event) => setBranch(event.target.value)}
              value={branch}
            />
          </label>
        </CardBody>
      </Card>

      <Card>
        <CardBody className="grid gap-3 sm:grid-cols-2">
          <div className="grid gap-2 rounded-lg border border-border p-3">
            <div className="flex items-center justify-between gap-2">
              <span className="text-sm font-bold text-strong-foreground">
                {t('onboarding.gitCheck')}
              </span>
              <Button
                onClick={() => void handleCheckGit()}
                size="sm"
                variant="secondary"
              >
                {t('onboarding.checkGit')}
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              {gitCheck
                ? gitCheck.available
                  ? t('onboarding.gitOk', { version: gitCheck.version })
                  : t('onboarding.gitMissing')
                : '—'}
            </p>
          </div>
          <div className="grid gap-2 rounded-lg border border-border p-3">
            <div className="flex items-center justify-between gap-2">
              <span className="text-sm font-bold text-strong-foreground">
                {t('onboarding.remoteCheck')}
              </span>
              <Button
                disabled={!remote.trim()}
                onClick={() => void handleCheckRemote()}
                size="sm"
                variant="secondary"
              >
                {t('onboarding.checkRemote')}
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              {remoteCheck
                ? remoteCheck.ok
                  ? t('onboarding.remoteOk')
                  : t('onboarding.remoteFail', { message: remoteCheck.message })
                : '—'}
            </p>
          </div>
        </CardBody>
      </Card>

      <Button
        className="justify-self-start"
        loading={saving}
        onClick={() => void handleSave()}
      >
        {t('onboarding.save')}
      </Button>
    </div>
  )
}
