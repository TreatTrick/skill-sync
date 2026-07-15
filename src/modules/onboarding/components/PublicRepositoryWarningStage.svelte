<script lang="ts">
  import { ExternalLink, TriangleAlert } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import { Button, Card, CardContent } from '@/shared/ui'

  import type { GithubRepository } from '../schemas/onboarding'

  interface Props {
    repository: GithubRepository
    installUrl: string | null
    createRepositoryUrl: string
    busy: boolean
    onContinue: () => void
    onOpenExternal: (event: MouseEvent, url: string) => void
  }

  let {
    repository,
    installUrl,
    createRepositoryUrl,
    busy,
    onContinue,
    onOpenExternal,
  }: Props = $props()
</script>

<Card>
  <CardContent class="grid gap-4 pt-6">
    <div class="flex items-start gap-3">
      <TriangleAlert class="mt-0.5 size-5 shrink-0 text-warning" />
      <div class="grid gap-1.5">
        <h2 class="font-semibold text-strong-foreground">
          {t('github.publicRepositoryTitle')}
        </h2>
        <p class="text-sm text-muted-foreground">
          {t('github.publicRepositoryDescription', {
            repository: `${repository.owner}/${repository.repo}`,
          })}
        </p>
      </div>
    </div>

    <div class="flex flex-col gap-2 sm:flex-row sm:flex-wrap">
      <Button disabled={busy} loading={busy} onclick={onContinue}>
        {t('github.continuePublicRepository')}
      </Button>
      <Button
        onclick={(event: MouseEvent) => onOpenExternal(event, createRepositoryUrl)}
        variant="outline"
      >
        {t('github.createAnotherRepository')} <ExternalLink class="size-4" />
      </Button>
      {#if installUrl}
        <Button
          onclick={(event: MouseEvent) => onOpenExternal(event, installUrl ?? '')}
          variant="outline"
        >
          {t('github.adjustInstallation')} <ExternalLink class="size-4" />
        </Button>
      {/if}
    </div>
  </CardContent>
</Card>
