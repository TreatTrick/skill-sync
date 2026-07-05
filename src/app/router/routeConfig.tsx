import { LayoutDashboard } from 'lucide-react'
import type { ComponentType, ReactElement } from 'react'

import { DemoDashboardPage } from '@/modules/demo-dashboard'
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
    group: t('routeGroups.demo'),
    element: <DemoDashboardPage />,
    icon: LayoutDashboard,
  },
]

export const DEFAULT_ROUTE_PATH = '/app/dashboard'
