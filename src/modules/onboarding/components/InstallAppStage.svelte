<script lang="ts">
  import { ExternalLink, Check, LockKeyhole } from '@lucide/svelte'
  import { t } from '@/shared/i18n'
  import { Button } from '@/shared/ui'
  interface Props { stage: 'install_app' | 'repository_scope_blocked'; installUrl: string | null; busy: boolean; onOpenExternal: (event: MouseEvent, url: string) => void; onCheckInstallation: () => void }
  let { stage, installUrl, busy, onOpenExternal, onCheckInstallation }: Props = $props()
</script>
<div class="eyebrow">{t('onboarding.progress', { current: 3, total: 6 })}</div>
<h1>{stage === 'repository_scope_blocked' ? t('github.scopeTitle') : t('github.installTitle')}</h1>
<p class="lede">{stage === 'repository_scope_blocked' ? t('github.adjustScope') : t('github.installDescription')}</p>
<div class="gh">
  <span class="cap">{t('github.installExample')}</span>
  <div class="opt"><span class="radio"></span><span class="no">{t('github.allRepositories')}</span><span class="note bad">{t('github.doNotChoose')}</span></div>
  <div class="opt"><span class="radio on"></span><span>{t('github.onlySelectedRepository')}</span><span class="note good">{t('github.chooseThis')}</span></div>
  <div><span class="pick"><LockKeyhole class="ic sm" />{t('github.vaultExampleName')}</span> <span class="muted text-xs">{t('github.selectVaultExample')}</span></div>
</div>
<div class="perm"><span><Check class="ic sm" />{t('github.contentsPermission')}</span><span><Check class="ic sm" />{t('github.metadataPermission')}</span><span>{t('github.noOtherPermissions')}</span></div>
<div class="acts">
  {#if installUrl}<Button class="btn lg" onclick={(event: MouseEvent) => onOpenExternal(event, installUrl)} variant="outline">{stage === 'repository_scope_blocked' ? t('github.adjustInstallation') : t('github.installApp')} <ExternalLink class="ic sm" /></Button>{/if}
  <Button class="btn primary lg" disabled={busy} loading={busy} onclick={onCheckInstallation}>{t(stage === 'repository_scope_blocked' ? 'sync.recheck' : 'github.checkInstallation')}</Button>
</div>
