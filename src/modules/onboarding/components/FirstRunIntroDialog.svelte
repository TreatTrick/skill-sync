<script lang="ts">
  import { onMount } from 'svelte'

  import { INTRO_SEEN_STORAGE_KEY } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
  } from '@/shared/ui'
  import LanguageToggle from './LanguageToggle.svelte'

  // First-run concept intro. Shown until dismissed; the seen flag is persisted
  // to localStorage so returning users go straight into the setup flow. The
  // flag is reset by the settings "disconnect GitHub" flow so a fresh
  // onboarding re-shows this intro. The `shown` guard prevents the initial
  // closed state from writing the flag before the dialog has ever been
  // displayed.

  let open = $state(false)
  let shown = $state(false)

  onMount(() => {
    if (window.localStorage.getItem(INTRO_SEEN_STORAGE_KEY) !== 'true') open = true
  })

  $effect(() => {
    if (open) {
      shown = true
    } else if (shown) {
      window.localStorage.setItem(INTRO_SEEN_STORAGE_KEY, 'true')
    }
  })

  const continueSetup = (): void => {
    open = false
  }
</script>

<Dialog bind:open>
  <DialogContent class="sm:max-w-md">
    <DialogHeader>
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div class="flex items-center gap-3">
          <span
            class="flex size-11 shrink-0 items-center justify-center rounded-full bg-foreground text-background"
            aria-hidden="true"
          >
            <svg viewBox="0 0 16 16" fill="currentColor" class="size-6">
              <path
                d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"
              />
            </svg>
            <span class="sr-only">{t('onboarding.intro.githubMark')}</span>
          </span>
          <div class="grid gap-1">
            <DialogTitle>{t('onboarding.intro.title')}</DialogTitle>
            <DialogDescription>{t('onboarding.intro.description')}</DialogDescription>
          </div>
        </div>
        <LanguageToggle />
      </div>
    </DialogHeader>

    <div class="grid gap-4">
      <div class="grid gap-1.5">
        <p class="text-sm font-medium text-strong-foreground">
          {t('onboarding.intro.vaultTitle')}
        </p>
        <p class="text-sm text-muted-foreground">{t('onboarding.intro.vaultDescription')}</p>
      </div>

      <p class="text-sm text-muted-foreground">{t('onboarding.intro.privateHint')}</p>

      <div class="grid gap-2">
        <p class="text-sm font-medium text-strong-foreground">
          {t('onboarding.intro.nextStepsTitle')}
        </p>
        <ol class="grid gap-1.5">
          <li class="flex items-start gap-2 text-sm text-muted-foreground">
            <span
              class="flex size-5 shrink-0 items-center justify-center rounded-full bg-primary-muted text-xs font-semibold text-primary-muted-foreground"
            >1</span>
            <span>{t('onboarding.intro.nextSteps.authorize')}</span>
          </li>
          <li class="flex items-start gap-2 text-sm text-muted-foreground">
            <span
              class="flex size-5 shrink-0 items-center justify-center rounded-full bg-primary-muted text-xs font-semibold text-primary-muted-foreground"
            >2</span>
            <span>{t('onboarding.intro.nextSteps.selectRepository')}</span>
          </li>
          <li class="flex items-start gap-2 text-sm text-muted-foreground">
            <span
              class="flex size-5 shrink-0 items-center justify-center rounded-full bg-primary-muted text-xs font-semibold text-primary-muted-foreground"
            >3</span>
            <span>{t('onboarding.intro.nextSteps.bind')}</span>
          </li>
        </ol>
      </div>
    </div>

    <DialogFooter>
      <Button class="w-full sm:w-auto" onclick={continueSetup}>
        {t('onboarding.intro.continue')}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
