import { FolderSync, Settings as SettingsIcon } from '@lucide/svelte'
import type { Component } from 'svelte'

type RouteTitleKey = 'routes.settings' | 'routes.sync'

export interface AppRouteConfig {
  path: string
  // i18n key resolved at render time so the nav follows language switches
  title: RouteTitleKey
  icon: Component<{ class?: string }>
}

export const appRoutes: AppRouteConfig[] = [
  {
    path: '/app/sync',
    title: 'routes.sync',
    icon: FolderSync,
  },
  {
    path: '/app/settings',
    title: 'routes.settings',
    icon: SettingsIcon,
  },
]
