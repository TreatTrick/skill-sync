const STORAGE_KEY = 'skill-sync-site-theme'

// The initial theme is applied by an inline script in app.html before paint.
// This helper toggles it imperatively; the toggle button icon is CSS-driven via
// the .dark class on <html>, so no reactive state is needed.
export const toggleTheme = (): void => {
  const isDark = document.documentElement.classList.toggle('dark')
  try {
    localStorage.setItem(STORAGE_KEY, isDark ? 'dark' : 'light')
  } catch {
    // ignore storage errors
  }
}
