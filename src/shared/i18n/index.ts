import i18next from 'i18next'
import { initReactI18next } from 'react-i18next'

import zhCN from './locales/zh-CN.json'

export const DEFAULT_LANGUAGE = 'zh-CN'

void i18next.use(initReactI18next).init({
  lng: DEFAULT_LANGUAGE,
  fallbackLng: DEFAULT_LANGUAGE,
  interpolation: {
    escapeValue: false,
  },
  resources: {
    [DEFAULT_LANGUAGE]: {
      translation: zhCN,
    },
  },
})

export const t = i18next.t.bind(i18next)

export { i18next }
