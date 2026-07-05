import i18next from 'i18next'
import { initReactI18next } from 'react-i18next'

import enUS from './locales/en-US.json'
import zhCN from './locales/zh-CN.json'
import { DEFAULT_LANGUAGE, readStoredLanguage } from './language'

void i18next.use(initReactI18next).init({
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

export const t = i18next.t.bind(i18next)

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
