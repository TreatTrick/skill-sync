<script lang="ts">
  import { t } from '@/shared/i18n'
  import { languageState } from '@/shared/state'
</script>
<div class="lang" role="group" aria-label={t('common.language')}>
  <button type="button" class:on={languageState.language === 'zh-CN'} aria-pressed={languageState.language === 'zh-CN'} onclick={() => void languageState.setLanguage('zh-CN')}>{t('common.languageShortZh')}</button>
  <button type="button" class:on={languageState.language === 'en-US'} aria-pressed={languageState.language === 'en-US'} onclick={() => void languageState.setLanguage('en-US')}>{t('common.languageShortEn')}</button>
</div>
