<script lang="ts">
  import { Input } from '@/shared/ui'
  import { t } from '@/shared/i18n'
  import type { AppConfig } from '@/shared/schemas'
  import { TriangleAlert } from '@lucide/svelte'

  interface Props {
    limits: AppConfig['limits']
    limitsInvalid: boolean
  }

  let { limits = $bindable(), limitsInvalid }: Props = $props()

  const MEBIBYTE = 1024 * 1024
  type ByteLimit = 'max_skill_zip_bytes' | 'max_single_file_unpacked_bytes' | 'max_skill_unpacked_bytes'

  let zipMiB = $state(String(limits.max_skill_zip_bytes / MEBIBYTE))
  let singleFileMiB = $state(String(limits.max_single_file_unpacked_bytes / MEBIBYTE))
  let skillTotalMiB = $state(String(limits.max_skill_unpacked_bytes / MEBIBYTE))

  const saveMiB = (key: ByteLimit, value: string): void => {
    const mebibytes = Number(value)
    if (!value.trim() || !Number.isFinite(mebibytes) || mebibytes <= 0) return
    const bytes = Math.round(mebibytes * MEBIBYTE)
    if (!Number.isSafeInteger(bytes) || bytes <= 0) return
    limits[key] = bytes
  }
</script>
<div class="srows">
  <div class="srow">
    <div class="lab"><b>{t('settings.maxSkillZipBytes')}</b><small>{t('settings.limitHelp.zip')}</small></div>
    <div class="ctl"><label class="unit"><Input aria-label={t('settings.maxSkillZipBytes')} value={zipMiB} min="0" step="any" type="number" oninput={(event) => { zipMiB = event.currentTarget.value; saveMiB('max_skill_zip_bytes', zipMiB) }} /><span>{t('settings.mebibyte')}</span></label></div>
  </div>
  <div class="srow">
    <div class="lab"><b>{t('settings.maxSkillFiles')}</b><small>{t('settings.limitHelp.files')}</small></div>
    <div class="ctl"><label class="unit"><Input aria-label={t('settings.maxSkillFiles')} bind:value={limits.max_skill_files} min="1" step="1" type="number" /><span>{t('settings.fileUnit')}</span></label></div>
  </div>
  <div class="srow">
    <div class="lab"><b>{t('settings.maxSingleFileUnpacked')}</b><small>{t('settings.limitHelp.single')}</small></div>
    <div class="ctl"><label class="unit" class:bad={limitsInvalid}><Input aria-invalid={limitsInvalid} aria-label={t('settings.maxSingleFileUnpacked')} value={singleFileMiB} min="0" step="any" type="number" oninput={(event) => { singleFileMiB = event.currentTarget.value; saveMiB('max_single_file_unpacked_bytes', singleFileMiB) }} /><span>{t('settings.mebibyte')}</span></label></div>
  </div>
  <div class="srow">
    <div class="lab"><b>{t('settings.maxSkillUnpacked')}</b><small>{t('settings.limitHelp.total')}</small></div>
    <div class="ctl"><label class="unit" class:bad={limitsInvalid}><Input aria-invalid={limitsInvalid} aria-label={t('settings.maxSkillUnpacked')} value={skillTotalMiB} min="0" step="any" type="number" oninput={(event) => { skillTotalMiB = event.currentTarget.value; saveMiB('max_skill_unpacked_bytes', skillTotalMiB) }} /><span>{t('settings.mebibyte')}</span></label></div>
  </div>
  <div class="srow">
    <div class="lab"><b>{t('settings.maxAutoDelete')}</b><small>{t('settings.limitHelp.autoDelete')}</small></div>
    <div class="ctl"><label class="unit"><Input aria-label={t('settings.maxAutoDelete')} bind:value={limits.max_auto_delete} min="0" step="1" type="number" /><span>{t('sync.selectedUnit')}</span></label></div>
  </div>
  {#if limitsInvalid}<div class="err" role="alert"><TriangleAlert class="size-4" />{t('settings.singleLimitTooLarge')}</div>{/if}
</div>
