<script lang="ts">
  import { ExternalLink } from '@lucide/svelte'
  import { t } from '@/shared/i18n'
  import { Button } from '@/shared/ui'
  interface Props { createRepositoryUrl: string; onOpenExternal: (event: MouseEvent, url: string) => void; onContinue: () => void }
  let { createRepositoryUrl, onOpenExternal, onContinue }: Props = $props()
</script>
<div class="eyebrow">{t('onboarding.progress', { current: 2, total: 6 })}</div>
<h1>{t('github.createRepositoryTitle')}</h1>
<p class="lede">{t('github.createRepositoryDescription')}</p>
<div class="box">
  <div class="todo"><span class="n">1</span><b>{t('github.createRepository')}</b><p>{t('github.createRepositoryHelp')}</p><div class="a"><Button class="btn" onclick={(event: MouseEvent) => onOpenExternal(event, createRepositoryUrl)} variant="outline">{t('github.createRepositoryOpen')} <ExternalLink class="ic sm" /></Button></div></div>
  <div class="todo"><span class="n">2</span><b>{t('github.returnAfterCreate')}</b><p>{t('github.existingRepositoryHint')}</p></div>
</div>
<div class="acts"><Button class="btn primary lg" onclick={onContinue}>{t('github.createRepositoryDone')}</Button></div>
