import { FolderSync, Settings as SettingsIcon } from '@lucide/svelte'
import type { Component } from 'svelte'

import { isWorkspaceReady } from '@/shared/lib'

export { isWorkspaceReady }

type RouteTitleKey = 'routes.settings' | 'routes.sync'

export type RouteGroupKey = 'routeGroups.main'

export interface AppRouteConfig {
  path: string
  // i18n key resolved at render time so the nav follows language switches
  title: RouteTitleKey
  group: RouteGroupKey
  icon: Component<{ class?: string }>
}

export const appRoutes: AppRouteConfig[] = [
  {
    path: '/app/sync',
    title: 'routes.sync',
    group: 'routeGroups.main',
    icon: FolderSync,
  },
  {
    path: '/app/settings',
    title: 'routes.settings',
    group: 'routeGroups.main',
    icon: SettingsIcon,
  },
]

export const DEFAULT_ROUTE_PATH = '/app/sync'
