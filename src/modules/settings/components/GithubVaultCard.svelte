<script lang="ts">
  import { RefreshCw } from '@lucide/svelte'
  import { t } from '@/shared/i18n'
  import type { AppConfig, AppState } from '@/shared/schemas'
  import { Button, GithubMark } from '@/shared/ui'
  interface Props {
    config: AppConfig
    appState: AppState | undefined
    onReconfigure: () => void
  }
  let { config, appState, onReconfigure }: Props = $props()
  const credentialStatusLabelKeys = {
    disconnected: 'settings.credentialDisconnected',
    valid: 'settings.credentialValid',
    refreshing: 'settings.credentialRefreshing',
    reauthorization_required: 'settings.credentialReauthorizationRequired',
  } as const
</script>

<div class="srows">
  <div class="vcard">
    <div>
      <div class="repo"><GithubMark class="ic" />
        {#if config.remote}{config.remote.owner}/{config.remote.repo}<span class="tag">{config.remote.branch}</span>{/if}
      </div>
      <div class="sub">
        <span>{#if appState?.credential_status === 'valid'}<span class="led"></span>{/if}
          {appState ? t(credentialStatusLabelKeys[appState.credential_status]) : t('settings.remoteCommitEmpty')}
          {#if appState?.github_user} · @{appState.github_user}{/if}
        </span>
        {#if appState?.device_name}<span>{t('settings.deviceMeta', { name: appState.device_name })}</span>{/if}
        <span>{t('settings.remoteCommit')} <span class="mono">{appState?.remote_commit?.slice(0, 7) ?? t('settings.remoteCommitEmpty')}</span></span>
      </div>
    </div>
    <Button class="btn" onclick={onReconfigure} variant="outline"><RefreshCw class="size-4" />{t('settings.reconfigureVault')}</Button>
  </div>
  <details class="tech">
    <summary>{t('settings.technicalDetails')}</summary>
    <dl class="kv">
      <dt>{t('settings.githubAppSlug')}</dt><dd><span class="mono">{appState?.github_app_slug ?? '—'}</span></dd>
      {#if config.remote}
        <dt>{t('settings.installationId')}</dt><dd><span class="mono">{config.remote.installation_id}</span></dd>
        <dt>{t('settings.repositoryId')}</dt><dd><span class="mono">{config.remote.repository_id}</span></dd>
      {/if}
      <dt>{t('settings.deviceId')}</dt><dd><span class="mono">{config.device_id}</span></dd>
      <dt>{t('settings.remoteCommit')}</dt><dd><span class="mono">{appState?.remote_commit ?? '—'}</span></dd>
    </dl>
  </details>
</div>
