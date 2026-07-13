<script lang="ts">
  import { t } from '@/shared/i18n'
  import type { AppConfig } from '@/shared/schemas'
  import { Card, CardContent, CardDescription, CardHeader, CardTitle, Input } from '@/shared/ui'

  interface Props {
    limits: AppConfig['limits']
    limitsInvalid: boolean
  }

  let { limits = $bindable(), limitsInvalid }: Props = $props()

  const formatBytes = (value: number): string => {
    if (value >= 1024 * 1024) return `${Math.round(value / 1024 / 1024)} MiB`
    if (value >= 1024) return `${Math.round(value / 1024)} KiB`
    return `${value} B`
  }
</script>

<Card>
  <CardHeader>
    <CardTitle>{t('settings.limits')}</CardTitle>
    <CardDescription>{t('settings.limitsDescription')}</CardDescription>
  </CardHeader>
  <CardContent class="grid gap-4 sm:grid-cols-2">
    <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
      {t('settings.maxSkillZipBytes')}
      <Input bind:value={limits.max_skill_zip_bytes} min="1" step="1" type="number" />
      <span class="text-xs font-normal">{formatBytes(limits.max_skill_zip_bytes)}</span>
    </label>
    <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
      {t('settings.maxSkillFiles')}
      <Input bind:value={limits.max_skill_files} min="1" step="1" type="number" />
    </label>
    <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
      {t('settings.maxSingleFileUnpacked')}
      <Input bind:value={limits.max_single_file_unpacked_bytes} min="1" step="1" type="number" />
      <span class="text-xs font-normal">{formatBytes(limits.max_single_file_unpacked_bytes)}</span>
    </label>
    <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
      {t('settings.maxSkillUnpacked')}
      <Input bind:value={limits.max_skill_unpacked_bytes} min="1" step="1" type="number" />
      <span class="text-xs font-normal">{formatBytes(limits.max_skill_unpacked_bytes)}</span>
    </label>
    <label class="grid gap-1.5 text-sm font-medium text-muted-foreground">
      {t('settings.maxAutoDelete')}
      <Input bind:value={limits.max_auto_delete} min="0" step="1" type="number" />
    </label>
    {#if limitsInvalid}
      <p class="text-sm text-destructive sm:col-span-2">{t('settings.singleLimitTooLarge')}</p>
    {/if}
  </CardContent>
</Card>
