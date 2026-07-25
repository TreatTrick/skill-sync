<script lang="ts">
  import { Eye, GitMerge, KeyRound, Shield } from '@lucide/svelte'
  import { i18n } from '../i18n.svelte'

  const features = [
    {
      icon: Shield,
      titleKey: 'features.localfirst.title',
      descKey: 'features.localfirst.desc',
    },
    {
      icon: Eye,
      titleKey: 'features.preview.title',
      descKey: 'features.preview.desc',
    },
    {
      icon: GitMerge,
      titleKey: 'features.conflicts.title',
      descKey: 'features.conflicts.desc',
    },
    {
      icon: KeyRound,
      titleKey: 'features.nogit.title',
      descKey: 'features.nogit.desc',
    },
  ]
</script>

<section class="border-t border-border bg-surface">
  <div class="mx-auto max-w-6xl px-4 py-16 sm:px-6 sm:py-20">
    <h2
      class="text-center text-3xl font-bold tracking-tight text-strong-foreground"
    >
      {i18n.t('features.title')}
    </h2>
    <p class="mx-auto mt-4 max-w-2xl text-center text-muted-foreground">
      {i18n.t('features.subtitle')}
    </p>
    <div class="mt-12 grid gap-5 sm:grid-cols-2 lg:grid-cols-4">
      {#each features as feature (feature.titleKey)}
        {@const Icon = feature.icon}
        <div
          class="rounded-xl border border-border bg-background p-6 shadow-sm transition-shadow hover:shadow-md"
        >
          <div
            class="flex h-10 w-10 items-center justify-center rounded-lg bg-primary-muted text-primary"
          >
            <Icon class="h-5 w-5" />
          </div>
          <h3 class="mt-4 text-base font-semibold text-foreground">
            {i18n.t(feature.titleKey)}
          </h3>
          <p class="mt-2 text-sm leading-relaxed text-muted-foreground">
            {i18n.t(feature.descKey)}
          </p>
        </div>
      {/each}
    </div>
  </div>
</section>
