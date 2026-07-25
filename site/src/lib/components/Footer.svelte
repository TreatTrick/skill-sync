<script lang="ts">
  import { i18n } from '../i18n.svelte'
  import { REPO_URL, RELEASES_URL } from '../links'
</script>

<footer class="border-t border-border">
  <div
    class="mx-auto flex max-w-6xl flex-col items-center justify-between gap-4 px-4 py-8 sm:flex-row sm:px-6"
  >
    <p class="text-sm text-muted-foreground">
      {i18n.t('footer.license')}
    </p>
    <nav class="flex items-center gap-4 text-sm">
      <a
        href={REPO_URL}
        class="text-muted-foreground transition-colors hover:text-foreground"
      >
        {i18n.t('nav.github')}
      </a>
      <a
        href={RELEASES_URL}
        class="text-muted-foreground transition-colors hover:text-foreground"
      >
        {i18n.t('nav.releases')}
      </a>
    </nav>
  </div>
</footer>
