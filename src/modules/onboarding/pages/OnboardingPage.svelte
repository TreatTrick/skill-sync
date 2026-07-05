<script lang="ts">
  import { createQuery, useQueryClient } from '@tanstack/svelte-query'
  import { goto } from '$app/navigation'

  import { errorMessage } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import { Button, Card, CardBody, CardHeader, Input } from '@/shared/ui'
  import { getAppState } from '@/modules/settings'

  import { checkGit, checkRemote, prepareRepo } from '../api/onboardingApi'
  import type { GitCheck, RemoteCheck } from '../schemas/onboarding'

  const queryClient = useQueryClient()
  const appState = createQuery(() => ({
    queryKey: ['app-state'],
    queryFn: getAppState,
  }))
  let remote = $state('')
  let branch = $state('main')
  let gitCheck = $state<GitCheck | null>(null)
  let remoteCheck = $state<RemoteCheck | null>(null)
  let msg = $state('')
  let saving = $state(false)
  let prefilled = $state(false)

  // Prefill from the loaded app state once it arrives.
  $effect(() => {
    if (appState.data && !prefilled) {
      prefilled = true
      remote = appState.data.config.repository.remote
      branch = appState.data.config.repository.branch || 'main'
    }
  })

  const handleCheckGit = async () => {
    try {
      gitCheck = await checkGit()
    } catch (error) {
      msg = errorMessage(error)
    }
  }

  const handleCheckRemote = async () => {
    try {
      remoteCheck = await checkRemote(remote)
    } catch (error) {
      msg = errorMessage(error)
    }
  }

  const handleSave = async () => {
    if (gitCheck && !gitCheck.available) {
      msg = t('onboarding.needGit')
      return
    }
    saving = true
    msg = ''
    try {
      await prepareRepo('', remote, branch)
      await queryClient.invalidateQueries({ queryKey: ['app-state'] })
      void goto('/app/dashboard')
    } catch (error) {
      msg = errorMessage(error)
    } finally {
      saving = false
    }
  }
</script>

<div class="grid max-w-2xl gap-4">
  <Card>
    <CardHeader description={t('onboarding.description')} title={t('onboarding.title')} />
  </Card>

  {#if msg}
    <Card class="border-destructive-border bg-destructive-muted">
      <CardBody class="text-sm text-destructive">{msg}</CardBody>
    </Card>
  {/if}

  <Card>
    <CardBody class="grid gap-4">
      <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
        {t('onboarding.remote')}
        <Input bind:value={remote} />
      </label>
      <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
        {t('onboarding.branch')}
        <Input bind:value={branch} />
      </label>
    </CardBody>
  </Card>

  <Card>
    <CardBody class="grid gap-3 sm:grid-cols-2">
      <div class="grid gap-2 rounded-lg border border-border p-3">
        <div class="flex items-center justify-between gap-2">
          <span class="text-sm font-bold text-strong-foreground">
            {t('onboarding.gitCheck')}
          </span>
          <Button onclick={() => void handleCheckGit()} size="sm" variant="secondary">
            {t('onboarding.checkGit')}
          </Button>
        </div>
        <p class="text-xs text-muted-foreground">
          {#if gitCheck}
            {gitCheck.available
              ? t('onboarding.gitOk', { version: gitCheck.version })
              : t('onboarding.gitMissing')}
          {:else}
            —
          {/if}
        </p>
      </div>
      <div class="grid gap-2 rounded-lg border border-border p-3">
        <div class="flex items-center justify-between gap-2">
          <span class="text-sm font-bold text-strong-foreground">
            {t('onboarding.remoteCheck')}
          </span>
          <Button
            disabled={!remote.trim()}
            onclick={() => void handleCheckRemote()}
            size="sm"
            variant="secondary"
          >
            {t('onboarding.checkRemote')}
          </Button>
        </div>
        <p class="text-xs text-muted-foreground">
          {#if remoteCheck}
            {remoteCheck.ok
              ? t('onboarding.remoteOk')
              : t('onboarding.remoteFail', { message: remoteCheck.message })}
          {:else}
            —
          {/if}
        </p>
      </div>
    </CardBody>
  </Card>

  <Button class="justify-self-start" loading={saving} onclick={() => void handleSave()}>
    {t('onboarding.save')}
  </Button>
</div>
