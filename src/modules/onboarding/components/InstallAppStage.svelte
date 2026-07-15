<script lang="ts">
  import { ExternalLink } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import { Button, Card, CardContent } from '@/shared/ui'

  interface Props {
    stage: 'install_app' | 'repository_scope_blocked'
    installUrl: string | null
    createRepositoryUrl: string
    busy: boolean
    onOpenExternal: (event: MouseEvent, url: string) => void
    onCheckInstallation: () => void
  }

  let {
    stage,
    installUrl,
    createRepositoryUrl,
    busy,
    onOpenExternal,
    onCheckInstallation,
  }: Props = $props()
</script>

<Card>
  <CardContent class="grid gap-4 pt-6">
    <h2 class="font-semibold text-strong-foreground">{t('github.installTitle')}</h2>
    <p class="text-sm text-muted-foreground">
      {stage === 'repository_scope_blocked' ? t('github.adjustScope') : t('github.installDescription')}
    </p>
    {#if stage === 'install_app'}
      <Button
        class="w-fit"
        onclick={(event: MouseEvent) => onOpenExternal(event, createRepositoryUrl)}
        variant="outline"
      >
        {t('github.createRepository')} <ExternalLink class="size-4" />
      </Button>
    {/if}
    {#if installUrl}
      <Button
        class="w-fit"
        onclick={(event: MouseEvent) => onOpenExternal(event, installUrl ?? '')}
        variant="outline"
      >
        {stage === 'repository_scope_blocked'
          ? t('github.adjustInstallation')
          : t('github.installApp')} <ExternalLink class="size-4" />
      </Button>
    {/if}
    <Button disabled={busy} loading={busy} onclick={onCheckInstallation} variant="outline">
      {t('github.checkInstallation')}
    </Button>
  </CardContent>
</Card>
