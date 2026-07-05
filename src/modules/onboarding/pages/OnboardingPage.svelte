<script lang="ts">
  import { createQuery, useQueryClient } from '@tanstack/svelte-query'
  import { goto } from '$app/navigation'

  import { errorMessage } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import {
    Button,
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
    Input,
  } from '@/shared/ui'
  import { getAppState } from '@/modules/settings'

  import { checkGit, checkRemote, prepareRepo } from '../api/onboardingApi'
  import type { GitCheck, RemoteCheck } from '../schemas/onboarding'
  import SshSetupDialog from '../components/SshSetupDialog.svelte'

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
  let sshDialogOpen = $state(false)

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

<div class="grid gap-4">
  <Card>
    <CardHeader class="flex-row items-center justify-between space-y-0">
      <div class="space-y-1.5">
        <CardTitle>{t('onboarding.title')}</CardTitle>
        <CardDescription>{t('onboarding.description')}</CardDescription>
      </div>
      <Button loading={saving} onclick={() => void handleSave()}>
        {t('onboarding.save')}
      </Button>
    </CardHeader>
  </Card>

  {#if msg}
    <Card class="border-destructive-border bg-destructive-muted">
      <CardContent class="text-sm text-destructive pt-6">{msg}</CardContent>
    </Card>
  {/if}

  <Card>
    <CardContent class="grid gap-4 pt-6">
      <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
        {t('onboarding.remote')}
        <Input bind:value={remote} />
      </label>
      <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
        {t('onboarding.branch')}
        <Input bind:value={branch} />
      </label>
    </CardContent>
  </Card>

  <Card>
    <CardContent class="flex items-center justify-between gap-2 pt-6">
      <span class="text-sm font-bold text-strong-foreground">
        {t('onboarding.sshHintTitle')}
      </span>
      <Button
        onclick={() => (sshDialogOpen = true)}
        size="sm"
        variant="outline"
      >
        {t('onboarding.sshHintToggle')}
      </Button>
    </CardContent>
  </Card>
  <SshSetupDialog bind:open={sshDialogOpen} />

  <Card>
    <CardContent class="grid gap-3 pt-6 sm:grid-cols-2">
      <div class="grid gap-2 rounded-lg border border-border p-3">
        <div class="flex items-center justify-between gap-2">
          <span class="text-sm font-bold text-strong-foreground">
            {t('onboarding.gitCheck')}
          </span>
          <Button onclick={() => void handleCheckGit()} size="sm" variant="outline">
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
            variant="outline"
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
    </CardContent>
  </Card>
</div>
