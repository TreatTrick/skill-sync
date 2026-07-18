<script lang="ts">
  import { t } from '@/shared/i18n'
  import { languageState } from '@/shared/state'
  import { cn } from '@/shared/lib'
</script>

<div class="flex items-center gap-1 text-sm" role="group" aria-label={t('common.language')}>
  <button
    type="button"
    class={cn(
      'rounded-md px-2 py-1 font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/40',
      languageState.language === 'zh-CN'
        ? 'text-foreground'
        : 'text-muted-foreground hover:bg-surface-hover hover:text-foreground',
    )}
    aria-pressed={languageState.language === 'zh-CN'}
    onclick={() => void languageState.setLanguage('zh-CN')}
  >
    {t('common.languageShortZh')}
  </button>
  <span class="text-muted-foreground" aria-hidden="true">|</span>
  <button
    type="button"
    class={cn(
      'rounded-md px-2 py-1 font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-ring/40',
      languageState.language === 'en-US'
        ? 'text-foreground'
        : 'text-muted-foreground hover:bg-surface-hover hover:text-foreground',
    )}
    aria-pressed={languageState.language === 'en-US'}
    onclick={() => void languageState.setLanguage('en-US')}
  >
    {t('common.languageShortEn')}
  </button>
</div>
