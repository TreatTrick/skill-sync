<script lang="ts">
  import { i18n } from '../i18n.svelte'

  const steps = [
    { num: '1', titleKey: 'how.step1.title', descKey: 'how.step1.desc' },
    { num: '2', titleKey: 'how.step2.title', descKey: 'how.step2.desc' },
    { num: '3', titleKey: 'how.step3.title', descKey: 'how.step3.desc' },
  ]
</script>

<section class="border-t border-border">
  <div class="mx-auto max-w-6xl px-4 py-16 sm:px-6 sm:py-20">
    <h2
      class="text-center text-3xl font-bold tracking-tight text-strong-foreground"
    >
      {i18n.t('how.title')}
    </h2>
    <ol class="mt-12 grid gap-6 sm:grid-cols-3">
      {#each steps as step (step.num)}
        <li class="rounded-xl border border-border bg-surface p-6">
          <div
            class="flex h-9 w-9 items-center justify-center rounded-full bg-primary text-sm font-bold text-primary-foreground"
          >
            {step.num}
          </div>
          <h3 class="mt-4 text-base font-semibold text-foreground">
            {i18n.t(step.titleKey)}
          </h3>
          <p class="mt-2 text-sm leading-relaxed text-muted-foreground">
            {i18n.t(step.descKey)}
          </p>
        </li>
      {/each}
    </ol>
  </div>
</section>
