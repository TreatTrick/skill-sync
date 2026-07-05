export type Language = 'zh-CN' | 'en-US'

export const DEFAULT_LANGUAGE: Language = 'zh-CN'

export const SUPPORTED_LANGUAGES: readonly Language[] = ['zh-CN', 'en-US']

export const LANGUAGE_STORAGE_KEY = 'skill-sync.language'

export const readStoredLanguage = (): Language => {
  const stored = window.localStorage.getItem(LANGUAGE_STORAGE_KEY)
  return stored === 'zh-CN' || stored === 'en-US' ? stored : DEFAULT_LANGUAGE
}
