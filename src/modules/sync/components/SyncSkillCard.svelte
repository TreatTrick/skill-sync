<script lang="ts">
  import { AlertTriangle, ChevronRight } from '@lucide/svelte'

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
    statusLabelKey,
    statusTone,
  } from '../lib/syncStatus'
  import type { SyncSkillEntry } from '../schemas/syncPlan'

  interface Props {
    entry: SyncSkillEntry
    selected?: boolean
    requiresConfirmation?: boolean
    onToggle?: (selected: boolean) => void
    onOpenConflict?: () => void
  }

  let {
    entry,
    selected = false,
    requiresConfirmation = false,
    onToggle,
    onOpenConflict,
  }: Props = $props()

  const shortHash = (hash: string | null): string => {
    if (!hash) return t('sync.notAvailable')
    return hash.length > 12 ? hash.slice(0, 12) : hash
  }

  const deleteLabelKey = (
    entry: SyncSkillEntry,
  ): 'sync.deleteLocal' | 'sync.deleteRemote' | null => {
    if (entry.delete_direction === 'delete_local') return 'sync.deleteLocal'
    if (entry.delete_direction === 'delete_remote') return 'sync.deleteRemote'
    return null
  }

</script>

<Card class="h-full">
  <CardContent class="grid gap-3 p-4">
    <div class="flex items-start gap-3">
      {#if onToggle}
        <Checkbox
          aria-label={t('sync.selectAction', { name: entry.name })}
          checked={selected}
          onCheckedChange={(checked) => onToggle?.(checked === true)}
        />
      {/if}
      <div class="min-w-0 flex-1">
        <div class="flex flex-wrap items-center gap-2">
          <h3 class="truncate text-base font-bold text-strong-foreground">
            {entry.name}
          </h3>
          <Badge variant="secondary">{entry.namespace}</Badge>
          <StatusBadge tone={statusTone(entry.status)}>
            {t(statusLabelKey(entry.status))}
          </StatusBadge>
        </div>
        <p class="mt-1 truncate text-xs text-muted-foreground">{entry.skill_id}</p>
      </div>
    </div>

    <div class="grid grid-cols-1 gap-2 text-xs text-muted-foreground sm:grid-cols-3">
      <div class="truncate">
        <span class="text-foreground">{t('sync.entry.localHash')}:</span>
        {shortHash(entry.local_hash)}
      </div>
      <div class="truncate">
        <span class="text-foreground">{t('sync.entry.remoteHash')}:</span>
        {shortHash(entry.remote_hash)}
      </div>
      <div class="truncate">
        <span class="text-foreground">{t('sync.entry.baseHash')}:</span>
        {shortHash(entry.base_hash)}
      </div>
    </div>

    <div class="grid gap-1 text-xs text-muted-foreground">
      <div class="truncate">
        <span class="text-foreground">{t('sync.entry.path')}:</span>
        {entry.local_path ?? entry.relative_dir ?? t('sync.notAvailable')}
      </div>
      <div class="truncate">
        <span class="text-foreground">{t('sync.entry.folder')}:</span>
        {entry.folder_name}
      </div>
    </div>

    {#if entry.delete_direction}
      <StatusBadge tone="warning">
        {t(deleteLabelKey(entry) ?? 'sync.entry.delete')}
      </StatusBadge>
    {/if}

    {#if requiresConfirmation}
      <div class="flex items-center gap-2 text-xs font-medium text-warning">
        <AlertTriangle class="size-4 shrink-0" />
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
