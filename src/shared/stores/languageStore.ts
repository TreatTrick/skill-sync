import { create } from 'zustand'

import { i18next } from '@/shared/i18n'
import {
  LANGUAGE_STORAGE_KEY,
  readStoredLanguage,
  type Language,
} from '@/shared/i18n/language'

interface LanguageStoreState {
  language: Language
  setLanguage: (language: Language) => void
}

export const useLanguageStore = create<LanguageStoreState>((set) => ({
  language: readStoredLanguage(),
  setLanguage: (language) => {
    window.localStorage.setItem(LANGUAGE_STORAGE_KEY, language)
    void i18next.changeLanguage(language)
    set({ language })
  },
}))
