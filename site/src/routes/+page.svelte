<script lang="ts">
  import DownloadSection from '../lib/components/DownloadSection.svelte'
  import FeatureCards from '../lib/components/FeatureCards.svelte'
  import Hero from '../lib/components/Hero.svelte'
  import HowItWorks from '../lib/components/HowItWorks.svelte'
  import { i18n } from '../lib/i18n.svelte'
</script>

<svelte:head>
  <title>{i18n.t('page.title')}</title>
</svelte:head>

<Hero />
<FeatureCards />
<HowItWorks />
<DownloadSection />
