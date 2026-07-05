<script lang="ts">
  import { applyTheme, themeState } from '@/shared/state/theme.svelte'

  // Re-subscribe whenever the selected theme changes; only "system" reacts to
  // OS-level color-scheme changes.
  $effect(() => {
    if (themeState.theme !== 'system') {
      return
    }
    const mql = window.matchMedia('(prefers-color-scheme: dark)')
    const handler = () => applyTheme('system')
    mql.addEventListener('change', handler)
    return () => mql.removeEventListener('change', handler)
  })
</script>
