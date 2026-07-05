import { NavLink, Outlet, useLocation } from 'react-router-dom'
import { PanelLeftClose, PanelLeftOpen } from 'lucide-react'

import { cn } from '@/shared/lib'
import { t } from '@/shared/i18n'
import { useUiStore } from '@/shared/stores'
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/shared/ui'

import { appRoutes, type RouteGroupKey } from '../router/routeConfig'

const routeGroups = appRoutes.reduce<Record<string, typeof appRoutes>>(
  (groups, route) => {
    groups[route.group] = [...(groups[route.group] ?? []), route]
    return groups
  },
  {},
)

export const AppLayout = () => {
  const location = useLocation()
  const collapsed = useUiStore((state) => state.sidebarCollapsed)
  const setSidebarCollapsed = useUiStore((state) => state.setSidebarCollapsed)
  const currentRoute =
    appRoutes.find((route) => route.path === location.pathname) ?? appRoutes[0]

  return (
    <div
      className={cn(
        'grid min-h-screen grid-cols-1 bg-background text-foreground transition-[grid-template-columns] duration-200',
        collapsed
          ? 'lg:grid-cols-[64px_minmax(0,1fr)]'
          : 'lg:grid-cols-[260px_minmax(0,1fr)]',
      )}
    >
      <aside className="flex flex-col border-b border-border bg-surface lg:sticky lg:top-0 lg:h-screen lg:border-b-0 lg:border-r">
        <div
          className={cn(
            'flex min-h-16 shrink-0 items-center gap-3 px-4 py-4',
            collapsed && 'lg:justify-center lg:px-0',
          )}
        >
          <img
            alt=""
            className="size-9 shrink-0 rounded-lg shadow-sm"
            src="/favicon.svg"
          />
          <div className={cn('min-w-0', collapsed && 'lg:hidden')}>
            <div className="truncate text-base font-extrabold text-strong-foreground">
              {t('layout.brandTitle')}
            </div>
            <div className="mt-0.5 truncate text-xs text-muted-foreground">
              {t('layout.brandSubtitle')}
            </div>
          </div>
        </div>

        <nav
          aria-label={t('layout.navLabel')}
          className="grid flex-1 content-start gap-4 px-3 pb-4 pt-2 sm:grid-cols-2 lg:grid-cols-1 lg:overflow-auto"
        >
          {Object.entries(routeGroups).map(([group, routes]) => (
            <div className="grid gap-1" key={group}>
              <div
                className={cn(
                  'px-2.5 pb-1 text-xs font-bold uppercase tracking-wide text-muted-foreground',
                  collapsed && 'lg:hidden',
                )}
              >
                {t(group as RouteGroupKey)}
              </div>
              {routes.map((route) => {
                const Icon = route.icon

                return (
                  <NavLink
                    className={({ isActive }) =>
                      cn(
                        'flex h-9 items-center gap-2.5 rounded-lg px-2.5 text-sm font-medium transition-colors',
                        collapsed && 'lg:justify-center lg:px-0',
                        isActive
                          ? 'bg-primary-muted font-bold text-primary-muted-foreground'
                          : 'text-foreground hover:bg-surface-hover',
                      )
                    }
                    key={route.path}
                    title={collapsed ? t(route.title) : undefined}
                    to={route.path}
                  >
                    <Icon className="size-4 shrink-0" />
                    <span className={cn('truncate', collapsed && 'lg:hidden')}>
                      {t(route.title)}
                    </span>
                  </NavLink>
                )
              })}
            </div>
          ))}
        </nav>

        <div className="hidden shrink-0 px-3 pb-4 pt-2 lg:block">
          <button
            aria-label={collapsed ? t('layout.expand') : t('layout.collapse')}
            className={cn(
              'flex h-9 w-full items-center gap-2.5 rounded-lg px-2.5 text-sm font-medium text-muted-foreground transition-colors hover:bg-surface-hover',
              collapsed && 'lg:justify-center lg:px-0',
            )}
            onClick={() => setSidebarCollapsed(!collapsed)}
            title={collapsed ? t('layout.expand') : t('layout.collapse')}
            type="button"
          >
            {collapsed ? (
              <PanelLeftOpen className="size-4 shrink-0" />
            ) : (
              <PanelLeftClose className="size-4 shrink-0" />
            )}
            {!collapsed ? (
              <span className="truncate">{t('layout.collapse')}</span>
            ) : null}
          </button>
        </div>
      </aside>

      <main className="min-w-0">
        <header className="sticky top-0 z-10 border-b border-border bg-background/80 backdrop-blur">
          <div className="mx-auto w-full max-w-screen-2xl px-4 py-4 sm:px-6">
            <Breadcrumb>
              <BreadcrumbList>
                <BreadcrumbItem>
                  <BreadcrumbLink href="/app">
                    {t('common.workspace')}
                  </BreadcrumbLink>
                </BreadcrumbItem>
                <BreadcrumbSeparator />
                <BreadcrumbItem>
                  <BreadcrumbPage>{t(currentRoute.title)}</BreadcrumbPage>
                </BreadcrumbItem>
              </BreadcrumbList>
            </Breadcrumb>
            <h1 className="mt-1.5 text-2xl font-bold text-strong-foreground">
              {t(currentRoute.title)}
            </h1>
          </div>
        </header>

        <div className="mx-auto w-full max-w-screen-2xl px-4 py-5 sm:px-6">
          <Outlet />
        </div>
      </main>
    </div>
  )
}
