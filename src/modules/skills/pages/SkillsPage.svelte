<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query'
  import { FolderOpen, Package, RefreshCw } from '@lucide/svelte'

  import { errorMessage, openPath } from '@/shared/lib'
  import { hostLabel, t } from '@/shared/i18n'
  import {
    Badge,
    Button,
    Card,
    CardContent,
    CardDescription,
    CardHeader,
    CardTitle,
    EmptyState,
    Spinner,
    StatusBadge,
  } from '@/shared/ui'

  import { scanSkills } from '../api/scanSkills'

  const shortHash = (hash: string) =>
    hash.length > 12 ? hash.slice(0, 12) : hash

  const scan = createQuery(() => ({
    queryKey: ['scan-skills'],
    queryFn: scanSkills,
  }))
  const skills = $derived(scan.data?.skills ?? [])
  const warnings = $derived(scan.data?.warnings ?? [])
</script>

<div class="grid gap-4">
  <Card>
    <CardHeader class="flex-row items-center justify-between space-y-0">
      <div class="space-y-1.5">
        <CardTitle>{t('skills.title')}</CardTitle>
        <CardDescription>{t('skills.description')}</CardDescription>
      </div>
      <Button
        loading={scan.isFetching}
        onclick={() => void scan.refetch()}
        variant="outline"
      >
        {#snippet icon()}
          <RefreshCw class="size-4" />
        {/snippet}
        {t('skills.rescan')}
      </Button>
    </CardHeader>
  </Card>

  {#if scan.isLoading}
    <div class="flex justify-center py-12">
      <Spinner class="size-6" />
    </div>
  {/if}

  {#if scan.error}
    <Card>
      <CardContent class="pt-6 text-sm text-destructive">
        {errorMessage(scan.error)}
      </CardContent>
    </Card>
  {/if}

  {#if warnings.length > 0}
    <Card class="border-warning-border bg-warning-muted">
      <CardContent class="pt-6 text-sm text-warning">
        <div class="font-bold">{t('skills.warnings')}</div>
        <ul class="mt-1 grid gap-1">
          {#each warnings as warning, index (index)}
            <li>{warning}</li>
          {/each}
        </ul>
      </CardContent>
    </Card>
  {/if}

  {#if skills.length === 0 && !scan.isLoading}
    <Card>
      <EmptyState title={t('skills.empty')}>
        {#snippet icon()}
          <Package class="size-10" />
        {/snippet}
      </EmptyState>
    </Card>
  {/if}

  <div class="grid grid-cols-1 gap-3 lg:grid-cols-2">
    {#each skills as skill (skill.id)}
      <Card class="p-4">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <div class="grid gap-1">
            <div class="text-base font-bold text-strong-foreground">
              {skill.name}
            </div>
            <Badge variant="secondary">{hostLabel(skill.host)}</Badge>
          </div>
          <StatusBadge tone="success">{t('skills.enabled')}</StatusBadge>
        </div>
        <p class="mt-2 text-sm text-muted-foreground">{skill.description}</p>
        <div
          class="mt-3 grid grid-cols-1 gap-1 text-xs text-muted-foreground sm:grid-cols-2"
        >
          <div class="truncate">
            <span class="text-foreground">{t('skills.columns.path')}:</span>
            {skill.source_path}
          </div>
          <div class="truncate">
            <span class="text-foreground">{t('skills.columns.modified')}:</span>
            {skill.modified_at || '—'}
          </div>
          <div class="truncate sm:col-span-2">
            <span class="text-foreground">{t('skills.columns.hash')}:</span>
            {shortHash(skill.hash)}
          </div>
        </div>
        <div class="mt-3 flex justify-end">
          <Button
            onclick={() => void openPath(skill.source_path)}
            size="sm"
            variant="outline"
          >
            {#snippet icon()}
              <FolderOpen class="size-3.5" />
            {/snippet}
            {t('skills.openFolder')}
          </Button>
        </div>
      </Card>
    {/each}
  </div>
</div>
