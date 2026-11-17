<script lang="ts">
  import { Check, Laptop, ArrowLeftRight } from '@lucide/svelte'
  import { t } from '@/shared/i18n'
  import { Button, GithubMark } from '@/shared/ui'
  interface Props { open: boolean; onContinue: () => void }
  let { open, onContinue }: Props = $props()
</script>
{#if open}
  <div class="panel">
    <h1>{t('onboarding.intro.title')}</h1>
    <p class="lede">{t('onboarding.intro.description')}</p>
    <div class="flow">
      <div class="node"><b><Laptop class="ic" />{t('onboarding.intro.thisComputer')}</b><small>~/.claude/skills<br />~/.codex/skills<br />~/.agents/skills</small></div>
      <div class="arr"><ArrowLeftRight class="ic lg" /></div>
      <div class="node v"><b><GithubMark />{t('onboarding.intro.vaultTitle')}</b><small>{t('onboarding.intro.privateHint')}<br />manifest.json + blobs/</small></div>
      <div class="arr"><ArrowLeftRight class="ic lg" /></div>
      <div class="node"><b><Laptop class="ic" />{t('onboarding.intro.otherComputer')}</b><small class="whitespace-pre-line">{t('onboarding.intro.otherComputerHint')}</small></div>
    </div>
    <div class="box"><b>{t('onboarding.intro.nextTitle')}</b><div class="perm"><span><Check class="ic sm" />{t('onboarding.intro.nextSteps.authorize')}</span><span><Check class="ic sm" />{t('onboarding.intro.nextSteps.selectRepository')}</span><span><Check class="ic sm" />{t('onboarding.intro.nextSteps.bind')}</span></div></div>
    <div class="acts"><Button class="btn primary lg" onclick={onContinue}>{t('onboarding.intro.continue')}</Button></div>
  </div>
{/if}
