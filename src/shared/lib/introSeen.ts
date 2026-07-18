// localStorage flag tracking whether the first-run intro dialog has been
// dismissed. The onboarding intro dialog reads it on mount; the settings
// "disconnect GitHub" flow resets it so a fresh onboarding re-shows the intro.
export const INTRO_SEEN_STORAGE_KEY = 'skill-sync.intro-seen'

// Clear the intro-seen flag so the first-run intro dialog shows again on the
// next onboarding entry (e.g. after disconnecting GitHub, with or without an
// app restart).
export const resetIntroSeen = (): void => {
  window.localStorage.removeItem(INTRO_SEEN_STORAGE_KEY)
}
