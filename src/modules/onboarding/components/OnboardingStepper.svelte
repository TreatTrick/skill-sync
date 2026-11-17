<script lang="ts">
  import { Check } from '@lucide/svelte'
  import { t } from '@/shared/i18n'
  interface Props { current: number; ariaLabel?: string; summaries?: string[] }
  let { current, ariaLabel, summaries = [] }: Props = $props()
  const steps = [
    'onboarding.stage.authorize',
    'onboarding.stage.createRepository',
    'onboarding.stage.installApp',
    'onboarding.stage.branch',
    'onboarding.stage.vault',
    'onboarding.stage.ready',
  ] as const
</script>
<ol class="steps" aria-label={ariaLabel}>
  {#each steps as step, i (step)}
    {@const number = i + 1}
    <li class:done={number < current} class:cur={number === current} aria-current={number === current ? 'step' : undefined}>
      <span class="mk">{#if number < current}<Check class="size-3.5" />{:else}{number}{/if}</span>
      <div><b>{t(step)}</b>{#if number < current && summaries[i]}<small>{summaries[i]}</small>{/if}</div>
    </li>
  {/each}
</ol>
