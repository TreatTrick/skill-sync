<script lang="ts">
  import { Copy, ExternalLink, LoaderCircle } from '@lucide/svelte'
  import { t } from '@/shared/i18n'
  import { Button, GithubMark } from '@/shared/ui'
  import { onMount } from 'svelte'
  import type { DeviceFlowStart } from '@/shared/schemas'
  interface Props {
    stage: 'authorize' | 'device_pending'; busy: boolean; deviceFlow: DeviceFlowStart | null; deviceExpiresAt: number | null
    onStart: () => void; onCopyCode: () => void; onOpenExternal: (event: MouseEvent, url: string) => void
  }
  let { stage, busy, deviceFlow, deviceExpiresAt, onStart, onCopyCode, onOpenExternal }: Props = $props()
  let now = $state(Date.now())
  const countdown = $derived.by(() => {
    const seconds = Math.max(0, Math.ceil(((deviceExpiresAt ?? now) - now) / 1000))
    return String(Math.floor(seconds / 60)).padStart(2, '0') + ':' + String(seconds % 60).padStart(2, '0')
  })
  onMount(() => {
    const timer = setInterval(() => (now = Date.now()), 1000)
    return () => clearInterval(timer)
  })
</script>
<div class="eyebrow">{t('onboarding.progress', { current: 1, total: 6 })}</div>
<h1>{stage === 'device_pending' ? t('github.deviceCodeTitle') : t('github.authorizeTitle')}</h1>
<p class="lede">{stage === 'device_pending' ? t('github.deviceCodeDescription') : t('github.authorizeDescription')}</p>
{#if stage === 'device_pending' && deviceFlow}
  <div class="box">
    <div class="code" aria-label={t('github.deviceCodeLabel')}>
      {#each deviceFlow.user_code.split('') as character, i (i)}{#if character === '-'}<i></i>{:else}<span>{character}</span>{/if}{/each}
      <Button class="btn sm" onclick={onCopyCode} variant="outline"><Copy class="ic sm" />{t('common.actions.copy')}</Button>
    </div>
    <div class="acts"><Button class="btn primary" onclick={(event: MouseEvent) => onOpenExternal(event, deviceFlow.verification_uri)} variant="outline">{t('github.openVerification')} <ExternalLink class="ic sm" /></Button></div>
    {#if deviceExpiresAt}<div class="wait"><LoaderCircle class="ic sm spin" />{t('github.waitingWithExpiry', { time: countdown })}</div>{/if}
  </div>
  <div class="acts"><Button class="btn ghost sm" disabled={busy} onclick={onStart} variant="ghost">{t('github.refreshCode')}</Button></div>
{:else}
  <div class="acts"><Button class="btn primary lg" disabled={busy} loading={busy} onclick={onStart}>{#if !busy}<GithubMark />{/if}{t('github.connectGithub')}</Button></div>
{/if}
