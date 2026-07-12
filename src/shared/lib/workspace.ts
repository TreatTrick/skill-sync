import type { AppState } from '@/shared/schemas'

export const isWorkspaceReady = (state: AppState): boolean =>
  state.configured &&
  state.github_authorized &&
  ['valid', 'refreshing'].includes(state.credential_status)
