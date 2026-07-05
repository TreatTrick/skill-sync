<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query'
  import { ListChecks } from '@lucide/svelte'

  import { cn, errorMessage } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import {
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
    EmptyState,
    Spinner,
    StatusBadge,
  } from '@/shared/ui'
  import { getSyncPlan, syncDecisions } from '@/modules/sync'

  const shortHash = (hash: string) =>
    hash.length > 12 ? hash.slice(0, 12) : hash

  const CHOICES = [
    { key: 'local', labelKey: 'conflicts.keepLocal' },
    { key: 'remote', labelKey: 'conflicts.useRemote' },
    { key: 'skip', labelKey: 'conflicts.skip' },
  ] as const

  const plan = createQuery(() => ({
    queryKey: ['sync-plan'],
    queryFn: getSyncPlan,
  }))
  const conflicts = $derived(plan.data?.conflicts ?? [])
</script>

<div class="grid gap-4">
  <Card>
    <CardHeader>
      <CardTitle>{t('conflicts.title')}</CardTitle>
      <CardDescription>{t('conflicts.description')}</CardDescription>
    </CardHeader>
  </Card>

  {#if plan.isLoading}
    <div class="flex justify-center py-12">
      <Spinner class="size-6" />
    </div>
  {/if}

  {#if plan.error}
    <Card class="border-destructive-border bg-destructive-muted">
      <CardContent class="text-sm text-destructive">
        {errorMessage(plan.error)}
      </CardContent>
    </Card>
  {/if}

  <p class="text-xs text-muted-foreground">{t('conflicts.reviewAtSync')}</p>

  {#if conflicts.length === 0 && !plan.isLoading}
    <Card>
      <EmptyState title={t('conflicts.empty')}>
        {#snippet icon()}
          <ListChecks class="size-10" />
        {/snippet}
      </EmptyState>
    </Card>
  {/if}

  <div class="grid grid-cols-1 gap-2 lg:grid-cols-2">
    {#each conflicts as conflict (conflict.skill_id)}
      {@const decision = syncDecisions.decisions[conflict.skill_id] ?? ''}
      <div
        class="grid gap-2 rounded-lg border border-warning-border bg-warning-muted p-3 text-sm"
      >
        <div class="flex flex-wrap items-center justify-between gap-2">
          <span class="font-bold text-strong-foreground">{conflict.name}</span>
          <StatusBadge tone="warning">{conflict.reason}</StatusBadge>
        </div>
        <div
          class="grid grid-cols-1 gap-1 text-xs text-muted-foreground sm:grid-cols-2"
        >
          <div class="truncate">
            {t('conflicts.localHash')}: {shortHash(conflict.local_hash)}
          </div>
          <div class="truncate">
            {t('conflicts.remoteHash')}: {shortHash(conflict.remote_hash)}
          </div>
        </div>
        <div class="flex flex-wrap gap-2">
          {#each CHOICES as choice (choice.key)}
            <button
              class={cn(
                'h-8 rounded-lg border px-2.5 text-xs font-medium transition-colors',
                decision === choice.key
                  ? 'border-primary bg-primary-muted text-primary-muted-foreground'
                  : 'border-border bg-surface text-foreground hover:bg-surface-hover',
              )}
              onclick={() =>
                syncDecisions.setDecision(conflict.skill_id, choice.key)}
              type="button"
            >
              {t(choice.labelKey)}
            </button>
          {/each}
        </div>
      </div>
    {/each}
  </div>
</div>
