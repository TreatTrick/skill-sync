import { useEffect } from 'react'

import { applyTheme, useThemeStore } from '@/shared/stores/themeStore'

// Listen for system theme changes; only active when "system" is selected
export const ThemeSync = () => {
  const theme = useThemeStore((state) => state.theme)

  useEffect(() => {
    if (theme !== 'system') {
      return
    }
    const mql = window.matchMedia('(prefers-color-scheme: dark)')
    const handler = () => applyTheme('system')
    mql.addEventListener('change', handler)
    return () => mql.removeEventListener('change', handler)
  }, [theme])

  return null
}
