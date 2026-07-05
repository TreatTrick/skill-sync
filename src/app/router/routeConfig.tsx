import {
  FolderSync,
  LayoutDashboard,
  ListChecks,
  Package,
  RotateCcw,
  Settings as SettingsIcon,
  Sparkles,
} from 'lucide-react'
import type { ComponentType, ReactElement } from 'react'

import { ConflictsPage } from '@/modules/conflicts'
import { DashboardPage } from '@/modules/dashboard'
import { BackupsPage } from '@/modules/backups'
import { OnboardingPage } from '@/modules/onboarding'
import { SettingsPage } from '@/modules/settings'
import { SkillsPage } from '@/modules/skills'
import { SyncPreviewPage } from '@/modules/sync'

export type RouteTitleKey =
  | 'routes.backups'
  | 'routes.conflicts'
  | 'routes.dashboard'
  | 'routes.onboarding'
  | 'routes.settings'
  | 'routes.skills'
  | 'routes.sync'

export type RouteGroupKey = 'routeGroups.main'

export interface AppRouteConfig {
  path: string
  // i18n key resolved at render time so the nav follows language switches
  title: RouteTitleKey
  group: RouteGroupKey
  element: ReactElement
  icon: ComponentType<{ className?: string }>
}

export const appRoutes: AppRouteConfig[] = [
  {
    path: '/app/dashboard',
    title: 'routes.dashboard',
    group: 'routeGroups.main',
    element: <DashboardPage />,
    icon: LayoutDashboard,
  },
  {
    path: '/app/skills',
    title: 'routes.skills',
    group: 'routeGroups.main',
    element: <SkillsPage />,
    icon: Package,
  },
  {
    path: '/app/sync',
    title: 'routes.sync',
    group: 'routeGroups.main',
    element: <SyncPreviewPage />,
    icon: FolderSync,
  },
  {
    path: '/app/conflicts',
    title: 'routes.conflicts',
    group: 'routeGroups.main',
    element: <ConflictsPage />,
    icon: ListChecks,
  },
  {
    path: '/app/backups',
    title: 'routes.backups',
    group: 'routeGroups.main',
    element: <BackupsPage />,
    icon: RotateCcw,
  },
  {
    path: '/app/settings',
    title: 'routes.settings',
    group: 'routeGroups.main',
    element: <SettingsPage />,
    icon: SettingsIcon,
  },
  {
    path: '/app/onboarding',
    title: 'routes.onboarding',
    group: 'routeGroups.main',
    element: <OnboardingPage />,
    icon: Sparkles,
  },
]

export const DEFAULT_ROUTE_PATH = '/app/dashboard'
