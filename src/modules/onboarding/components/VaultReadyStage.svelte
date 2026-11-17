<script lang="ts">
  import { Check, Globe, LockKeyhole } from '@lucide/svelte'
  import { t } from '@/shared/i18n'
  import { Button, Checkbox } from '@/shared/ui'
  import type { RemoteConfig } from '@/shared/schemas'
  interface Props { remote: RemoteConfig | null; user?: string | null; isPrivate: boolean | null; bindingChanged: boolean; busy: boolean; confirmRebind?: boolean; onBindVault: () => void }
  let { remote, user, isPrivate, bindingChanged, busy, onBindVault, confirmRebind = $bindable(false) }: Props = $props()
</script>
<div class="eyebrow">{t('onboarding.progress', { current: 6, total: 6 })}</div>
<div class="statusico"><Check class="ic lg" /></div>
<h1>{t('github.readyTitle')}</h1>
<p class="lede">{t('github.readyDescription')}</p>
<div class="box"><dl class="kv">
  <dt>{t('settings.repository')}</dt><dd><span class="mono">{remote ? remote.owner + '/' + remote.repo : '—'}</span>{#if isPrivate !== null}<span class="tag">{#if isPrivate}<LockKeyhole class="ic sm" />{:else}<Globe class="ic sm" />{/if}{t(isPrivate ? 'github.privateRepository' : 'github.publicRepository')}</span>{/if}</dd>
  <dt>{t('onboarding.branch')}</dt><dd><span class="mono">{remote?.branch ?? '—'}</span></dd>
  <dt>{t('settings.githubUser')}</dt><dd>{user ? '@' + user : '—'}</dd>
</dl></div>
{#if bindingChanged}<div class="callout"><label class="flex items-start gap-2"><Checkbox checked={confirmRebind} onCheckedChange={(checked) => (confirmRebind = checked === true)} /><span>{t('github.confirmRebind')}</span></label></div>{/if}
<div class="acts"><Button class="btn primary lg" disabled={busy || (bindingChanged && !confirmRebind)} loading={busy} onclick={onBindVault}>{t('github.bindVault')}</Button></div>
