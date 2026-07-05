// Reactive language state. Toggles the shared i18next singleton (initialized in
// @/shared/i18n); changing language re-renders any component reading
// languageState.language or calling t(...).
import i18next from 'i18next'

import {
  LANGUAGE_STORAGE_KEY,
  readStoredLanguage,
  type Language,
} from '@/shared/i18n/language'

class LanguageState {
  language = $state<Language>(readStoredLanguage())

  async setLanguage(language: Language): Promise<void> {
    window.localStorage.setItem(LANGUAGE_STORAGE_KEY, language)
    await i18next.changeLanguage(language)
    this.language = language
  }
}

export const languageState = new LanguageState()
