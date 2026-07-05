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
import { t } from '@/shared/i18n'

export interface AppRouteConfig {
  path: string
  title: string
  group: string
  element: ReactElement
  icon: ComponentType<{ className?: string }>
}

export const appRoutes: AppRouteConfig[] = [
  {
    path: '/app/dashboard',
    title: t('routes.dashboard'),
    group: t('routeGroups.main'),
    element: <DashboardPage />,
    icon: LayoutDashboard,
  },
  {
    path: '/app/skills',
    title: t('routes.skills'),
    group: t('routeGroups.main'),
    element: <SkillsPage />,
    icon: Package,
  },
  {
    path: '/app/sync',
    title: t('routes.sync'),
    group: t('routeGroups.main'),
    element: <SyncPreviewPage />,
    icon: FolderSync,
  },
  {
    path: '/app/conflicts',
    title: t('routes.conflicts'),
    group: t('routeGroups.main'),
    element: <ConflictsPage />,
    icon: ListChecks,
  },
  {
    path: '/app/backups',
    title: t('routes.backups'),
    group: t('routeGroups.main'),
    element: <BackupsPage />,
    icon: RotateCcw,
  },
  {
    path: '/app/settings',
    title: t('routes.settings'),
    group: t('routeGroups.main'),
    element: <SettingsPage />,
    icon: SettingsIcon,
  },
  {
    path: '/app/onboarding',
    title: t('routes.onboarding'),
    group: t('routeGroups.main'),
    element: <OnboardingPage />,
    icon: Sparkles,
  },
]

export const DEFAULT_ROUTE_PATH = '/app/dashboard'
