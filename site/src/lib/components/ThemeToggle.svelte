<script lang="ts">
  import { Moon, Sun } from '@lucide/svelte'
  import { i18n } from '../i18n.svelte'
  import { toggleTheme } from '../theme'
</script>

<button
  type="button"
  class="inline-flex h-9 w-9 items-center justify-center rounded-md border border-border text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
  aria-label={i18n.t('a11y.theme')}
  onclick={toggleTheme}
>
  <Moon class="h-5 w-5 dark:hidden" />
  <Sun class="hidden h-5 w-5 dark:block" />
</button>
