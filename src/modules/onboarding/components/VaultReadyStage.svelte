<script lang="ts">
  import { CheckCircle } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import { Button, Callout, Card, CardContent, Checkbox } from '@/shared/ui'
  import type { RemoteConfig } from '@/shared/schemas'

  interface Props {
    remote: RemoteConfig | null
    bindingChanged: boolean
    busy: boolean
    onBindVault: () => void
  }

  let {
    remote,
    bindingChanged,
    busy,
    onBindVault,
    confirmRebind = $bindable(false),
  }: Props & { confirmRebind?: boolean } = $props()
</script>

<Card>
  <CardContent class="grid gap-4 pt-6">
    <div class="flex items-center gap-3">
      <span class="flex size-10 shrink-0 items-center justify-center rounded-full bg-success-muted text-success">
        <CheckCircle class="size-5" />
      </span>
      <h2 class="font-semibold text-strong-foreground">{t('github.readyTitle')}</h2>
    </div>
    <p class="text-sm text-muted-foreground">{remote ? `${remote.owner}/${remote.repo} · ${remote.branch}` : ''}</p>
    {#if bindingChanged}
      <Callout tone="warning">
        <label class="flex items-start gap-2">
          <Checkbox
            checked={confirmRebind}
            onCheckedChange={(checked) => (confirmRebind = checked === true)}
          />
          <span>{t('github.confirmRebind')}</span>
        </label>
      </Callout>
    {/if}
    <Button disabled={busy} loading={busy} onclick={onBindVault}>
      {t('github.bindVault')}
    </Button>
  </CardContent>
</Card>
