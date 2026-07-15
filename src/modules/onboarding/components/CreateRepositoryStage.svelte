<script lang="ts">
  import { ExternalLink } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import { Button, Card, CardContent } from '@/shared/ui'

  interface Props {
    createRepositoryUrl: string
    onOpenExternal: (event: MouseEvent, url: string) => void
    onContinue: () => void
  }

  let { createRepositoryUrl, onOpenExternal, onContinue }: Props = $props()
</script>

<Card>
  <CardContent class="grid gap-4 pt-6">
    <h2 class="font-semibold text-strong-foreground">{t('github.createRepositoryTitle')}</h2>
    <p class="text-sm text-muted-foreground">{t('github.createRepositoryDescription')}</p>
    <Button
      class="w-fit"
      onclick={(event: MouseEvent) => onOpenExternal(event, createRepositoryUrl)}
      variant="outline"
    >
      {t('github.createRepository')} <ExternalLink class="size-4" />
    </Button>
    <Button onclick={onContinue}>
      {t('github.createRepositoryDone')}
    </Button>
  </CardContent>
</Card>
