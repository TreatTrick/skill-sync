<script lang="ts">
  import { i18n } from '../i18n.svelte'
</script>

<button
  type="button"
  class="inline-flex h-9 items-center justify-center rounded-md border border-border px-3 text-sm font-medium text-muted-foreground transition-colors hover:bg-surface-hover hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
  aria-label={i18n.t('a11y.language')}
  onclick={() => i18n.toggle()}
>
  {i18n.locale === 'en' ? '中文' : 'EN'}
</button>
