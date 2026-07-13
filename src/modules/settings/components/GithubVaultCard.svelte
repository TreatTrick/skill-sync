<script lang="ts">
  import { RefreshCw } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import type { AppConfig, AppState } from '@/shared/schemas'
  import {
    Button,
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
  } from '@/shared/ui'

  interface Props {
    config: AppConfig
    appState: AppState | undefined
    onReconfigure: () => void
    onDisconnect: () => void
  }

  let { config, appState, onReconfigure, onDisconnect }: Props = $props()

  const credentialStatusLabelKeys = {
    disconnected: 'settings.credentialDisconnected',
    valid: 'settings.credentialValid',
    refreshing: 'settings.credentialRefreshing',
    reauthorization_required: 'settings.credentialReauthorizationRequired',
  } as const
</script>

<Card>
  <CardHeader>
    <CardTitle>{t('settings.githubVault')}</CardTitle>
    <CardDescription>{t('settings.githubVaultReadOnly')}</CardDescription>
  </CardHeader>
  <CardContent class="text-sm">
    <dl class="divide-y divide-border-muted">
      <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
        <dt class="text-muted-foreground">{t('settings.githubAppSlug')}</dt>
        <dd class="font-mono text-foreground">{appState?.github_app_slug ?? t('settings.remoteCommitEmpty')}</dd>
      </div>
      <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
        <dt class="text-muted-foreground">{t('settings.githubUser')}</dt>
        <dd class="text-foreground">{appState?.github_user ?? t('settings.remoteCommitEmpty')}</dd>
      </div>
      <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
        <dt class="text-muted-foreground">{t('settings.credentialStatus')}</dt>
        <dd class="text-foreground">
          {#if appState}
            {t(credentialStatusLabelKeys[appState.credential_status])}
          {:else}
            {t('settings.remoteCommitEmpty')}
          {/if}
        </dd>
      </div>
      {#if config.remote}
        <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
          <dt class="text-muted-foreground">{t('settings.installationId')}</dt>
          <dd class="font-mono text-foreground">{config.remote.installation_id}</dd>
        </div>
        <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
          <dt class="text-muted-foreground">{t('settings.repositoryId')}</dt>
          <dd class="font-mono text-foreground">{config.remote.repository_id}</dd>
        </div>
        <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
          <dt class="text-muted-foreground">{t('settings.repository')}</dt>
          <dd class="font-mono text-foreground">{config.remote.owner}/{config.remote.repo}:{config.remote.branch}</dd>
        </div>
      {/if}
      <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
        <dt class="text-muted-foreground">{t('settings.deviceName')}</dt>
        <dd class="text-foreground">{appState?.device_name ?? config.device_id}</dd>
      </div>
      <div class="grid grid-cols-[140px_1fr] gap-3 py-2.5">
        <dt class="text-muted-foreground">{t('settings.remoteCommit')}</dt>
        <dd class="font-mono text-foreground">{appState?.remote_commit ?? t('settings.remoteCommitEmpty')}</dd>
      </div>
    </dl>
    <div class="flex flex-wrap gap-2 pt-2">
      <Button onclick={onReconfigure} size="sm" variant="outline">
        <RefreshCw class="size-4" />{t('settings.reconfigureVault')}
      </Button>
      <Button disabled={appState?.repository_id === null} onclick={onDisconnect} size="sm" variant="destructive">
        {t('settings.disconnectGithub')}
      </Button>
    </div>
  </CardContent>
</Card>
