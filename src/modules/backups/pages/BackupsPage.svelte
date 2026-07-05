<script lang="ts">
  import { createMutation, createQuery, useQueryClient } from '@tanstack/svelte-query'
  import { RotateCcw, Save } from '@lucide/svelte'

  import { errorMessage } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import {
    Button,
    Card,
    CardBody,
    CardHeader,
    EmptyState,
    Spinner,
  } from '@/shared/ui'

  import { listBackups, restoreBackup } from '../api/backupsApi'

  const formatSize = (bytes: number) => {
    if (bytes >= 1024 * 1024) {
      return `${(bytes / 1024 / 1024).toFixed(1)} MB`
    }
    if (bytes >= 1024) {
      return `${(bytes / 1024).toFixed(1)} KB`
    }
    return `${bytes} B`
  }

  const formatTime = (iso: string) => {
    if (!iso) {
      return '—'
    }
    const date = new Date(iso)
    return Number.isNaN(date.getTime()) ? iso : date.toLocaleString()
  }

  const queryClient = useQueryClient()
  const list = createQuery(() => ({
    queryKey: ['backups'],
    queryFn: listBackups,
  }))
  let msg = $state('')

  const restore = createMutation(() => ({
    mutationFn: (entry: { id: string; path: string }) =>
      restoreBackup(entry.id, entry.path),
    onSuccess: () => {
      msg = t('backups.restored')
      void queryClient.invalidateQueries({ queryKey: ['backups'] })
    },
    onError: (error) => {
      msg = t('backups.restoreError', { message: errorMessage(error) })
    },
  }))

  const backups = $derived(list.data ?? [])
</script>

<div class="grid gap-4">
  <Card>
    <CardHeader description={t('backups.description')} title={t('backups.title')} />
  </Card>

  {#if msg}
    <Card class="border-success-muted bg-success-muted">
      <CardBody class="flex items-center gap-2 text-sm text-success">
        <Save class="size-4 shrink-0" />
        {msg}
      </CardBody>
    </Card>
  {/if}

  {#if list.isLoading}
    <div class="flex justify-center py-12">
      <Spinner class="size-6" />
    </div>
  {/if}

  {#if backups.length === 0 && !list.isLoading}
    <Card>
      <EmptyState title={t('backups.empty')}>
        {#snippet icon()}
          <RotateCcw class="size-10" />
        {/snippet}
      </EmptyState>
    </Card>
  {/if}

  <div class="grid grid-cols-1 gap-2 lg:grid-cols-2">
    {#each backups as entry (entry.id)}
      <Card class="p-3">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <span class="font-bold text-strong-foreground">{entry.skill_id}</span>
          <Button
            disabled={restore.isPending}
            onclick={() =>
              restore.mutate({ id: entry.id, path: entry.original_path })}
            size="sm"
            variant="secondary"
          >
            {#snippet icon()}
              <RotateCcw class="size-3.5" />
            {/snippet}
            {t('backups.columns.actions')}
          </Button>
        </div>
        <div
          class="mt-2 grid grid-cols-1 gap-1 text-xs text-muted-foreground sm:grid-cols-2"
        >
          <div class="truncate">
            {t('backups.columns.time')}: {formatTime(entry.created_at)}
          </div>
          <div class="truncate">
            {t('backups.columns.size')}: {formatSize(entry.size)}
          </div>
          <div class="truncate sm:col-span-2">
            {t('backups.columns.path')}: {entry.original_path}
          </div>
        </div>
      </Card>
    {/each}
  </div>
</div>
