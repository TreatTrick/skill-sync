<script lang="ts">
  import { base } from '$app/paths'
  import { i18n } from '../i18n.svelte'
  import { LATEST_RELEASE_URL, REPO_URL } from '../links'
</script>

<section class="relative overflow-hidden">
  <div class="mx-auto max-w-6xl px-4 py-16 text-center sm:px-6 sm:py-20">
    <img
      src={`${base}/icon.png`}
      alt="Skill Sync"
      class="mx-auto h-20 w-20 rounded-2xl shadow-md"
    />
    <p
      class="mt-6 inline-flex items-center rounded-full border border-primary-border bg-primary-muted px-3 py-1 text-xs font-medium text-primary-muted-foreground"
    >
      {i18n.t('hero.badge')}
    </p>
    <h1
      class="mx-auto mt-6 max-w-3xl text-4xl font-bold tracking-tight text-strong-foreground sm:text-5xl"
    >
      {i18n.t('hero.title')}
    </h1>
    <div
      class="mx-auto mt-8 max-w-4xl overflow-hidden rounded-xl border border-border bg-surface shadow-lg"
    >
      {#key i18n.locale}
        <video
          class="aspect-video w-full object-contain"
          src={`${base}/video/${i18n.locale === 'zh' ? 'skillSyncChinese.mp4' : 'skillSyncEnglish.mp4'}`}
          autoplay
          muted
          loop
          playsinline
          aria-label={i18n.t('showcase.aria')}
        ></video>
      {/key}
    </div>
    <div
      class="mt-8 flex flex-col items-center justify-center gap-3 sm:flex-row"
    >
      <a
        href={LATEST_RELEASE_URL}
        class="inline-flex h-11 items-center justify-center rounded-md bg-primary px-6 text-sm font-semibold text-primary-foreground shadow-sm transition-colors hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        {i18n.t('hero.cta.download')}
      </a>
      <a
        href={REPO_URL}
        class="inline-flex h-11 items-center justify-center rounded-md border border-border bg-surface px-6 text-sm font-semibold text-foreground transition-colors hover:bg-surface-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        {i18n.t('hero.cta.github')}
      </a>
    </div>
    <p class="mt-6 text-sm text-muted-foreground">
      {i18n.t('hero.platforms')}
    </p>
  </div>
</section>
