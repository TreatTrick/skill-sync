import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useEffect, useState, type ReactNode } from 'react'

import { i18next } from '@/shared/i18n'
import { ThemeSync } from '@/shared/theme'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: 1,
      staleTime: 30_000,
    },
  },
})

interface AppProvidersProps {
  children: ReactNode
}

export const AppProviders = ({ children }: AppProvidersProps) => {
  // Re-render the whole tree when the i18n language changes so t() picks up new values.
  const [, setLanguageTick] = useState(0)
  useEffect(() => {
    const handler = () => setLanguageTick((n) => n + 1)
    i18next.on('languageChanged', handler)
    return () => i18next.off('languageChanged', handler)
  }, [])

  return (
    <QueryClientProvider client={queryClient}>
      <ThemeSync />
      {children}
    </QueryClientProvider>
  )
}
