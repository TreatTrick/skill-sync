import { en } from './locales/en'
import { zh } from './locales/zh'

export type Locale = 'en' | 'zh'

const STORAGE_KEY = 'skill-sync-site-lang'

const dictionaries: Record<Locale, Record<string, string>> = { en, zh }

const detectLocale = (): Locale => {
  if (typeof window === 'undefined') return 'en'
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (stored === 'en' || stored === 'zh') return stored
  } catch {
    // ignore storage errors
  }
  return navigator.language.toLowerCase().startsWith('zh') ? 'zh' : 'en'
}

const htmlLang = (locale: Locale): string => (locale === 'zh' ? 'zh-CN' : 'en')

class I18nState {
  locale = $state<Locale>(detectLocale())

  constructor() {
    if (typeof document !== 'undefined') {
      document.documentElement.lang = htmlLang(this.locale)
    }
  }

  // Arrow field so `this` stays bound; reading `this.locale` tracks the rune so
  // components calling i18n.t(...) re-render on locale change.
  t = (key: string): string =>
    dictionaries[this.locale][key] ?? dictionaries.en[key] ?? key

  setLocale(locale: Locale): void {
    this.locale = locale
    try {
      localStorage.setItem(STORAGE_KEY, locale)
    } catch {
      // ignore storage errors
    }
    if (typeof document !== 'undefined') {
      document.documentElement.lang = htmlLang(locale)
    }
  }

  toggle(): void {
    this.setLocale(this.locale === 'en' ? 'zh' : 'en')
  }
}

export const i18n = new I18nState()
