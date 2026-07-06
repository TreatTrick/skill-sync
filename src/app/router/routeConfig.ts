import {
  FolderSync,
  ListChecks,
  RotateCcw,
  Settings as SettingsIcon,
  Sparkles,
} from '@lucide/svelte'
import type { Component } from 'svelte'

export type RouteTitleKey =
  | 'routes.backups'
  | 'routes.conflicts'
  | 'routes.onboarding'
  | 'routes.settings'
  | 'routes.sync'

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
    path: '/app/conflicts',
    title: 'routes.conflicts',
    group: 'routeGroups.main',
    icon: ListChecks,
  },
  {
    path: '/app/backups',
    title: 'routes.backups',
    group: 'routeGroups.main',
    icon: RotateCcw,
  },
  {
    path: '/app/settings',
    title: 'routes.settings',
    group: 'routeGroups.main',
    icon: SettingsIcon,
  },
  {
    path: '/app/onboarding',
    title: 'routes.onboarding',
    group: 'routeGroups.main',
    icon: Sparkles,
  },
]

export const DEFAULT_ROUTE_PATH = '/app/sync'
