<script lang="ts">
  import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
  } from '@/shared/ui'
  import { t } from '@/shared/i18n'

  import {
    conflictDecisionOptions,
    decisionLabelKey,
  } from '../lib/syncStatus'
  import type { Conflict, SyncDecision } from '../schemas/syncPlan'

  interface Props {
    conflict: Conflict | null
    decision: SyncDecision | ''
    onDecision: (choice: SyncDecision) => void
  }

  let {
    conflict,
    decision,
    onDecision,
    open = $bindable(false),
  }: Props & { open?: boolean } = $props()

  const shortHash = (hash: string | null): string =>
    hash ? (hash.length > 12 ? hash.slice(0, 12) : hash) : t('sync.notAvailable')

  const choose = (choice: SyncDecision): void => {
    onDecision(choice)
    open = false
  }
</script>

<Dialog bind:open>
  <DialogContent class="sm:max-w-lg">
    {#if conflict}
      <DialogHeader>
        <DialogTitle>{conflict.name}</DialogTitle>
        <DialogDescription>{conflict.skill_id}</DialogDescription>
      </DialogHeader>

      <div class="grid gap-3">
        <div class="flex flex-wrap gap-2">
          <span class="rounded-md border border-border px-2 py-1 text-xs">
            {conflict.namespace}
          </span>
          <span class="rounded-md border border-warning-border bg-warning-muted px-2 py-1 text-xs text-warning">
            {t(`sync.conflictReason.${conflict.conflict_reason}`)}
          </span>
        </div>

        <div class="grid gap-2 text-sm text-muted-foreground">
          <div>
            <span class="text-foreground">{t('sync.entry.localHash')}:</span>
            {shortHash(conflict.local_hash)}
          </div>
          <div>
            <span class="text-foreground">{t('sync.entry.remoteHash')}:</span>
            {shortHash(conflict.remote_hash)}
          </div>
          <div>
            <span class="text-foreground">{t('sync.entry.baseHash')}:</span>
            {shortHash(conflict.base_hash)}
          </div>
          <div>
            <span class="text-foreground">{t('sync.entry.path')}:</span>
            {conflict.local_path ?? conflict.relative_dir ?? t('sync.notAvailable')}
          </div>
        </div>

        {#if conflict.warnings.length > 0}
          <ul class="grid gap-1 text-xs text-warning">
            {#each conflict.warnings as warning (warning)}
              <li>{warning}</li>
            {/each}
          </ul>
        {/if}

        <div class="grid gap-2">
          <p class="text-sm font-bold text-strong-foreground">
            {t('sync.conflictDetail.choose')}
          </p>
          <div class="grid gap-2 sm:grid-cols-3">
            {#each conflictDecisionOptions(conflict.conflict_reason) as choice (choice)}
              <Button
                onclick={() => choose(choice)}
                variant={decision === choice ? 'default' : 'outline'}
              >
                {t(decisionLabelKey(choice))}
              </Button>
            {/each}
          </div>
        </div>
      </div>

      <DialogFooter>
        <Button onclick={() => (open = false)} variant="ghost">
          {t('common.actions.cancel')}
        </Button>
      </DialogFooter>
    {/if}
  </DialogContent>
</Dialog>
