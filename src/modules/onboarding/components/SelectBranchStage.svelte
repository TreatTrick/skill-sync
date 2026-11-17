<script lang="ts">
  import { t } from '@/shared/i18n'
  import { Button, Select, SelectContent, SelectItem, SelectTrigger } from '@/shared/ui'
  import type { RemoteConfig } from '@/shared/schemas'
  interface Props { remote: RemoteConfig | null; branchNames: string[]; selectedBranch: string; busy: boolean; onChooseBranch: () => void }
  let { remote, branchNames, selectedBranch = $bindable(), busy, onChooseBranch }: Props = $props()
</script>
<div class="eyebrow">{t('onboarding.progress', { current: 4, total: 6 })}</div>
<h1>{t('github.selectBranch')}</h1>
<p class="lede">{t('github.branchDescription', { repository: remote ? remote.owner + '/' + remote.repo : t('github.repositoryUnavailable') })}</p>
<label class="grid justify-items-start gap-1.5"><b>{t('onboarding.branch')}</b>
  <Select type="single" bind:value={selectedBranch}><SelectTrigger class="sel">{selectedBranch}</SelectTrigger><SelectContent>{#each branchNames as branch (branch)}<SelectItem value={branch}>{branch}</SelectItem>{/each}</SelectContent></Select>
</label>
<div class="acts"><Button class="btn primary lg" disabled={busy || !selectedBranch} loading={busy} onclick={onChooseBranch}>{t('github.checkVault')}</Button></div>
