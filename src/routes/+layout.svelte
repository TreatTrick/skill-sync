<script lang="ts">
  import '@/index.css'
  import QueryProvider from '@/app/providers/QueryProvider.svelte'
  import { ThemeSync } from '@/shared/theme'
  import { initTheme } from '@/shared/state'

  let { children } = $props()

  // Apply theme synchronously before render to avoid first-frame flash
  initTheme()
</script>

<QueryProvider>
  <ThemeSync />
  {@render children?.()}
</QueryProvider>
