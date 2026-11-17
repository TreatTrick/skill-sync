<script lang="ts">
  import { ExternalLink, TriangleAlert } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import { Button } from '@/shared/ui'

  import type { GithubRepository } from '@/shared/schemas'

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
<div class="eyebrow">{t('onboarding.progress', { current: 4, total: 6 })}</div>
<div class="statusico warn"><TriangleAlert class="ic lg" /></div>
<h1>{t('github.publicRepositoryTitle', { repository: repository.owner + '/' + repository.repo })}</h1>
<p class="lede">{t('github.publicRepositoryDescription', { repository: repository.owner + '/' + repository.repo })}</p>
<div class="acts">
  <Button class="btn lg" onclick={(event: MouseEvent) => onOpenExternal(event, createRepositoryUrl)} variant="outline">{t('github.createAnotherRepository')} <ExternalLink class="ic sm" /></Button>
  {#if installUrl}<Button class="btn lg" onclick={(event: MouseEvent) => onOpenExternal(event, installUrl)} variant="outline">{t('github.adjustInstallation')} <ExternalLink class="ic sm" /></Button>{/if}
</div>
<div class="acts"><Button class="btn danger" disabled={busy} loading={busy} onclick={onContinue} variant="outline">{t('github.continuePublicRepository')}</Button></div>
