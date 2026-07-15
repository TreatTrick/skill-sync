<script lang="ts">
  import {
    ArrowDownToLine,
    ArrowUpFromLine,
    ChevronRight,
    FolderOpen,
    TriangleAlert,
    Undo2,
  } from '@lucide/svelte'

  import { cn } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import {
    Badge,
    Button,
    Card,
    CardContent,
    Checkbox,
    StatusBadge,
  } from '@/shared/ui'

  import {
    deleteDecisionOptions,
    decisionLabelKey,
    statusLabelKey,
    statusTone,
  } from '../lib/syncStatus'
  import type { SyncDecision, SyncSkillEntry } from '../schemas/syncPlan'

  interface Props {
    entry: SyncSkillEntry
    decision?: SyncDecision
    selected?: boolean
    requiresConfirmation?: boolean
    onToggle?: (selected: boolean) => void
    onDecision?: (choice: SyncDecision) => void
    onOpenConflict?: () => void
    onOpenFolder?: () => void
  }

  let {
    entry,
    decision,
    selected = false,
    requiresConfirmation = false,
    onToggle,
    onDecision,
    onOpenConflict,
    onOpenFolder,
  }: Props = $props()

  const recoveryDecision = $derived(
    deleteDecisionOptions(entry).find(
      (choice) => choice === 'restore_remote' || choice === 'keep_local',
    ),
  )
  const recoverySelected = $derived(
    recoveryDecision !== undefined && decision === recoveryDecision,
  )

  const deleteLabelKey = (
    entry: SyncSkillEntry,
  ): 'sync.deleteLocal' | 'sync.deleteRemote' | null => {
    if (entry.delete_direction === 'delete_local') return 'sync.deleteLocal'
    if (entry.delete_direction === 'delete_remote') return 'sync.deleteRemote'
    return null
  }

  const decisionTone = (
    choice: SyncDecision,
  ): 'neutral' | 'success' | 'warning' | 'destructive' | 'remote' => {
    if (choice === 'keep_local') return 'success'
    if (choice === 'use_remote' || choice === 'restore_remote') return 'remote'
    if (choice === 'delete_remote' || choice === 'accept_delete') {
      return 'destructive'
    }
    return 'neutral'
  }
</script>

<Card class={cn('h-full transition-shadow hover:shadow-md', (selected || decision) && 'border-primary bg-primary-muted/30')}>
  <CardContent class="grid gap-3 p-4">
    <div class="flex items-start gap-3">
      {#if onToggle}
        <Checkbox
          aria-label={t('sync.selectAction', { name: entry.name })}
          checked={selected}
          disabled={recoverySelected}
          onCheckedChange={(checked) => onToggle?.(checked === true)}
        />
      {/if}
      <div class="min-w-0 flex-1">
        <div class="flex flex-wrap items-center gap-2">
          <h3 class="truncate text-base font-semibold text-strong-foreground">
            {entry.name}
          </h3>
          <Badge variant="secondary">{entry.namespace}</Badge>
          <StatusBadge tone={statusTone(entry.status)}>
            {t(statusLabelKey(entry.status))}
          </StatusBadge>
          {#if decision}
            <StatusBadge tone={decisionTone(decision)}>
              {t(decisionLabelKey(decision))}
            </StatusBadge>
          {/if}
        </div>
        <p class="mt-1 truncate font-mono text-xs text-muted-foreground">{entry.skill_id}</p>
      </div>
    </div>

    <div class="grid gap-1 text-xs text-muted-foreground">
      <div class="truncate">
        <span class="text-foreground">{t('sync.entry.path')}:</span>
        <span class="font-mono">{entry.local_path ?? entry.relative_dir ?? t('sync.notAvailable')}</span>
      </div>
      <div class="flex items-center justify-between gap-2">
        <div class="min-w-0 truncate">
          <span class="text-foreground">{t('sync.entry.folder')}:</span>
          {entry.folder_name}
        </div>
        {#if onOpenFolder}
          <Button
            aria-label={t('skills.openFolder')}
            class="size-6 shrink-0 p-0"
            onclick={onOpenFolder}
            title={t('skills.openFolder')}
            variant="ghost"
          >
            <FolderOpen class="size-4" />
          </Button>
        {/if}
      </div>
    </div>

    {#if entry.delete_direction}
      <StatusBadge tone="warning">
        {t(deleteLabelKey(entry) ?? 'sync.entry.delete')}
      </StatusBadge>
    {/if}

    {#if recoveryDecision && onDecision}
      <Button
        class="w-full"
        onclick={() => onDecision?.(recoveryDecision)}
        size="sm"
        variant={recoverySelected ? 'outline' : 'default'}
      >
        {#if recoverySelected}
          <Undo2 class="size-4" />
          {t('sync.deleteRecovery.undo')}
        {:else if recoveryDecision === 'restore_remote'}
          <ArrowDownToLine class="size-4" />
          {t('sync.deleteRecovery.download')}
        {:else}
          <ArrowUpFromLine class="size-4" />
          {t('sync.deleteRecovery.upload')}
        {/if}
      </Button>
    {/if}

    {#if requiresConfirmation && !recoverySelected}
      <div class="flex items-center gap-2 text-xs font-medium text-warning">
        <TriangleAlert class="size-4 shrink-0" />
        {t('sync.confirmDelete')}
      </div>
    {/if}

    {#if entry.blocked_reason}
      <p class="text-xs text-destructive">
        {t('sync.entry.blocked')}: {entry.blocked_reason}
      </p>
    {/if}

    {#if entry.warnings.length > 0}
      <ul class="grid gap-1 text-xs text-warning">
        {#each entry.warnings as warning (warning)}
          <li>{warning}</li>
        {/each}
      </ul>
    {/if}

    {#if entry.conflict_reason && onOpenConflict}
      <Button
        class="w-full"
        onclick={onOpenConflict}
        size="sm"
        variant="outline"
      >
        {#snippet icon()}
          <ChevronRight class="size-4" />
        {/snippet}
        {t('sync.viewConflict')}
      </Button>
    {/if}
  </CardContent>
</Card>
