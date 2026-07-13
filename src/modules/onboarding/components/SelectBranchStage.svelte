<script lang="ts">
  import { t } from '@/shared/i18n'
  import { Button, Card, CardContent, Select, SelectContent, SelectItem, SelectTrigger } from '@/shared/ui'
  import type { RemoteConfig } from '../schemas/onboarding'

  interface Props {
    remote: RemoteConfig | null
    branchNames: string[]
    selectedBranch: string
    busy: boolean
    onChooseBranch: () => void
  }

  let {
    remote,
    branchNames,
    selectedBranch = $bindable(),
    busy,
    onChooseBranch,
  }: Props = $props()
</script>

<Card>
  <CardContent class="grid gap-4 pt-6">
    <h2 class="font-semibold text-strong-foreground">{t('github.selectBranch')}</h2>
    <p class="text-sm text-muted-foreground">
      {remote ? `${remote.owner}/${remote.repo}` : t('github.repositoryUnavailable')}
    </p>
    <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
      {t('onboarding.branch')}
      <Select type="single" bind:value={selectedBranch}>
        <SelectTrigger class="w-full">{selectedBranch}</SelectTrigger>
        <SelectContent>
          {#each branchNames as branch (branch)}
            <SelectItem value={branch}>{branch}</SelectItem>
          {/each}
        </SelectContent>
      </Select>
    </label>
    <Button disabled={busy || !selectedBranch} loading={busy} onclick={onChooseBranch}>
      {t('github.checkVault')}
    </Button>
  </CardContent>
</Card>
