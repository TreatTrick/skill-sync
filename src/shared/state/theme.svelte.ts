// Reactive theme state backed by localStorage. Replaces the Zustand themeStore
// for the Svelte UI; the storage key is unchanged so existing prefs carry over.
export type ThemeMode = 'light' | 'dark' | 'system'

const STORAGE_KEY = 'skill-sync.theme'

const readSystemTheme = (): 'light' | 'dark' =>
  window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'

const readStoredTheme = (): ThemeMode => {
  const stored = window.localStorage.getItem(STORAGE_KEY)
  return stored === 'light' || stored === 'dark' || stored === 'system'
    ? stored
    : 'system'
}

// Apply theme to the root element; called on runtime toggle
export const applyTheme = (mode: ThemeMode): void => {
  const effective = mode === 'system' ? readSystemTheme() : mode
  document.documentElement.classList.toggle('dark', effective === 'dark')
}

// Apply once synchronously at startup to avoid first-frame flash
export const initTheme = (): void => {
  applyTheme(readStoredTheme())
}

class ThemeState {
  theme = $state<ThemeMode>(readStoredTheme())

  setTheme(theme: ThemeMode): void {
    window.localStorage.setItem(STORAGE_KEY, theme)
    applyTheme(theme)
    this.theme = theme
  }
}

export const themeState = new ThemeState()
