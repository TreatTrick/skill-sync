import i18next from 'i18next'

import { languageState } from '@/shared/state/language.svelte'
import enUS from './locales/en-US.json'
import zhCN from './locales/zh-CN.json'
import { DEFAULT_LANGUAGE, readStoredLanguage } from './language'

void i18next.init({
  lng: readStoredLanguage(),
  fallbackLng: DEFAULT_LANGUAGE,
  interpolation: {
    escapeValue: false,
  },
  resources: {
    'zh-CN': { translation: zhCN },
    'en-US': { translation: enUS },
  },
})

// Bound TFunction; the wrapper reads languageState.language so Svelte components
// that call t(...) re-render on language change. Do not remove that read.
const translate = i18next.t.bind(i18next)

export const t = (
  ...args: Parameters<typeof translate>
): ReturnType<typeof translate> => {
  void languageState.language
  return translate(...args)
}

export { DEFAULT_LANGUAGE, SUPPORTED_LANGUAGES } from './language'
export type { Language } from './language'

export const hostLabel = (host: string): string => {
  if (host === 'codex') {
    return t('common.host.codex')
  }
  if (host === 'claude') {
    return t('common.host.claude')
  }
  return host
}

export { i18next }
