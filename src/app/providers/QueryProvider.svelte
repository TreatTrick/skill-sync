<script lang="ts">
  import { QueryClient, QueryClientProvider } from '@tanstack/svelte-query'

  let { children } = $props()

  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        refetchOnWindowFocus: false,
        retry: 1,
        staleTime: 30_000,
      },
    },
  })
</script>

<QueryClientProvider client={queryClient}>
  {@render children?.()}
</QueryClientProvider>
