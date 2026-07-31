<script lang="ts">
  import { base } from '$app/paths'
  import { i18n } from '../i18n.svelte'
  import { REPO_URL, RELEASES_URL } from '../links'
  import LangToggle from './LangToggle.svelte'
  import ThemeToggle from './ThemeToggle.svelte'
</script>

<header
  class="sticky top-0 z-50 border-b border-border bg-background/80 backdrop-blur"
>
  <div
    class="mx-auto flex h-16 max-w-6xl items-center justify-between px-4 sm:px-6"
  >
    <a href={base} class="flex items-center gap-2">
      <img src={`${base}/icon.png`} alt="Skill Sync" class="h-8 w-8" />
      <span class="text-base font-semibold text-foreground">
        {i18n.t('brand.name')}
      </span>
    </a>
    <nav class="flex items-center gap-2 sm:gap-4">
      <a
        href={RELEASES_URL}
        class="hidden text-sm font-medium text-muted-foreground transition-colors hover:text-foreground sm:inline-flex"
      >
        {i18n.t('nav.releases')}
      </a>
      <a
        href={REPO_URL}
        class="hidden text-sm font-medium text-muted-foreground transition-colors hover:text-foreground sm:inline-flex"
      >
        {i18n.t('nav.github')}
      </a>
      <LangToggle />
      <ThemeToggle />
    </nav>
  </div>
</header>
