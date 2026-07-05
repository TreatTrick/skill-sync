<script lang="ts">
  import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle,
    Tabs,
    TabsContent,
    TabsList,
    TabsTrigger,
  } from '@/shared/ui'
  import { t } from '@/shared/i18n'
  import sshPromptText from '../content/ssh-setup-prompt.md?raw'

  let { open = $bindable(false) } = $props()
  let promptCopied = $state(false)
  let activeTab = $state('ai')

  // Code blocks are commands/config snippets (no localized copy), so they stay
  // inline; only the surrounding prose goes through t(...). `as const` keeps the
  // title/desc keys as literals so the typed t(...) accepts them.
  const manualSteps = [
    {
      title: 'onboarding.sshManualStep1Title',
      desc: 'onboarding.sshManualStep1Desc',
      code: 'ssh-keygen -t ed25519 -C "skill-sync-personal" -f ~/.ssh/id_ed25519_personal',
    },
    {
      title: 'onboarding.sshManualStep2Title',
      desc: 'onboarding.sshManualStep2Desc',
      code: 'cat ~/.ssh/id_ed25519_personal.pub',
    },
    {
      title: 'onboarding.sshManualStep3Title',
      desc: 'onboarding.sshManualStep3Desc',
      code: 'Host github-personal\n    HostName github.com\n    User git\n    IdentityFile ~/.ssh/id_ed25519_personal\n    IdentitiesOnly yes',
    },
    {
      title: 'onboarding.sshManualStep4Title',
      desc: 'onboarding.sshManualStep4Desc',
      code: '# Windows: start the OpenSSH Authentication Agent service, then:\nssh-add ~/.ssh/id_ed25519_personal\n# macOS:\nssh-add --apple-use-keychain ~/.ssh/id_ed25519_personal\n# Linux:\nssh-add ~/.ssh/id_ed25519_personal',
    },
    {
      title: 'onboarding.sshManualStep5Title',
      desc: 'onboarding.sshManualStep5Desc',
      code: 'ssh -T git@github-personal',
    },
    {
      title: 'onboarding.sshManualStep6Title',
      desc: 'onboarding.sshManualStep6Desc',
      code: 'git@github-personal:<user>/<repo>.git',
    },
  ] as const

  const handleCopyPrompt = async () => {
    try {
      await navigator.clipboard.writeText(sshPromptText)
      promptCopied = true
      setTimeout(() => (promptCopied = false), 2500)
    } catch {
      // clipboard unavailable; the user can select the text manually
    }
  }
</script>

<Dialog bind:open>
  <DialogContent class="sm:max-w-2xl">
    <DialogHeader>
      <DialogTitle>{t('onboarding.sshHintTitle')}</DialogTitle>
      <DialogDescription>{t('onboarding.sshHintDescription')}</DialogDescription>
    </DialogHeader>

    <Tabs bind:value={activeTab} class="w-full">
      <TabsList class="flex w-full">
        <TabsTrigger value="ai">{t('onboarding.sshTabAi')}</TabsTrigger>
        <TabsTrigger value="manual">{t('onboarding.sshTabManual')}</TabsTrigger>
      </TabsList>

      <TabsContent value="ai" class="grid gap-3">
        <p class="text-sm text-muted-foreground">{t('onboarding.sshAiGuide')}</p>
        <pre
          class="max-h-72 overflow-auto rounded-lg border border-border bg-surface-muted p-3 font-mono text-xs text-foreground">{sshPromptText}</pre>
        <Button
          onclick={() => void handleCopyPrompt()}
          size="sm"
          class="justify-self-start"
        >
          {promptCopied
            ? t('onboarding.sshPromptCopied')
            : t('onboarding.copySshPrompt')}
        </Button>
      </TabsContent>

      <TabsContent value="manual" class="grid max-h-96 gap-3 overflow-y-auto">
        {#each manualSteps as step (step.title)}
          <div class="grid gap-1.5">
            <p class="text-sm font-bold text-strong-foreground">{t(step.title)}</p>
            <p class="text-xs text-muted-foreground">{t(step.desc)}</p>
            <pre
              class="overflow-auto rounded-lg border border-border bg-surface-muted p-3 font-mono text-xs">{step.code}</pre>
          </div>
        {/each}
      </TabsContent>
    </Tabs>
  </DialogContent>
</Dialog>
