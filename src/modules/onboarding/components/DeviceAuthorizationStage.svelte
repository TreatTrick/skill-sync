<script lang="ts">
  import { Copy, ExternalLink, KeyRound } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import { Button, Card, CardContent } from '@/shared/ui'
  import type { DeviceFlowStart } from '../schemas/onboarding'

  interface Props {
    stage: 'authorize' | 'device_pending'
    busy: boolean
    deviceFlow: DeviceFlowStart | null
    deviceExpiresAt: number | null
    onStart: () => void
    onCopyCode: () => void
    onOpenExternal: (event: MouseEvent, url: string) => void
  }

  let {
    stage,
    busy,
    deviceFlow,
    deviceExpiresAt,
    onStart,
    onCopyCode,
    onOpenExternal,
  }: Props = $props()
</script>

<Card>
  <CardContent class="grid gap-4 pt-6">
    <div class="flex items-center gap-3">
      <span class="flex size-10 shrink-0 items-center justify-center rounded-full bg-primary-muted text-primary-muted-foreground">
        <KeyRound class="size-5" />
      </span>
      <div class="grid gap-1">
        <h2 class="font-semibold text-strong-foreground">{t('github.authorizeTitle')}</h2>
        <p class="text-sm text-muted-foreground">{t('github.authorizeDescription')}</p>
      </div>
    </div>
    {#if stage === 'device_pending' && deviceFlow}
      <div class="grid gap-3 rounded-md border border-border bg-surface p-4">
        <p class="text-sm text-muted-foreground">{t('github.deviceCodeLabel')}</p>
        <div class="flex items-center gap-2">
          <code class="font-mono text-2xl font-bold tracking-widest text-strong-foreground">{deviceFlow.user_code}</code>
          <Button onclick={onCopyCode} size="icon" variant="outline">
            {#snippet icon()}
              <Copy class="size-4" />
            {/snippet}
            <span class="sr-only">{t('common.actions.copy')}</span>
          </Button>
        </div>
        <Button
          class="w-full justify-center"
          onclick={(event: MouseEvent) => onOpenExternal(event, deviceFlow?.verification_uri ?? '')}
          variant="outline"
        >
          {t('github.openVerification')} <ExternalLink class="size-4" />
        </Button>
        {#if deviceExpiresAt}
          <p class="text-xs text-muted-foreground">{t('github.waitingAuthorization')}</p>
        {/if}
      </div>
    {:else}
      <Button disabled={busy} loading={busy} onclick={onStart}>
        {t('github.connectGithub')}
      </Button>
    {/if}
  </CardContent>
</Card>
