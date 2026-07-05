import { NavLink, Outlet, useLocation } from 'react-router-dom'

import { t } from '@/shared/i18n'
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from '@/shared/ui'

import { appRoutes } from '../router/routeConfig'

const routeGroups = appRoutes.reduce<Record<string, typeof appRoutes>>(
  (groups, route) => {
    groups[route.group] = [...(groups[route.group] ?? []), route]
    return groups
  },
  {},
)

export const AppLayout = () => {
  const location = useLocation()
  const currentRoute =
    appRoutes.find((route) => route.path === location.pathname) ?? appRoutes[0]

  return (
    <div className="grid min-h-screen grid-cols-1 bg-background text-foreground lg:grid-cols-[264px_minmax(0,1fr)]">
      <aside className="border-b border-border bg-surface px-3.5 py-4 lg:sticky lg:top-0 lg:h-screen lg:overflow-auto lg:border-b-0 lg:border-r">
        <div className="flex min-h-13 items-center gap-3 px-1 pb-4">
          <div className="grid size-9 place-items-center rounded-lg bg-primary font-extrabold text-primary-foreground">
            {t('common.brandMark')}
          </div>
          <div>
            <div className="text-lg font-extrabold">
              {t('layout.brandTitle')}
            </div>
            <div className="mt-0.5 text-xs text-muted-foreground">
              {t('layout.brandSubtitle')}
            </div>
          </div>
        </div>

        <nav
          className="grid gap-5 pt-4 sm:grid-cols-2 lg:grid-cols-1"
          aria-label={t('layout.navLabel')}
        >
          {Object.entries(routeGroups).map(([group, routes]) => (
            <div className="grid gap-1.5" key={group}>
              <div className="px-2.5 pb-1 text-xs font-bold text-muted-foreground">
                {group}
              </div>
              {routes.map((route) => {
                const Icon = route.icon

                return (
                  <NavLink
                    className={({ isActive }) =>
                      [
                        'flex h-9 items-center gap-2.5 rounded-lg px-2.5 text-sm text-foreground',
                        isActive
                          ? 'bg-primary-muted font-bold text-primary-muted-foreground'
                          : 'hover:bg-surface-hover',
                      ].join(' ')
                    }
                    key={route.path}
                    to={route.path}
                  >
                    <Icon className="size-4" />
                    <span>{route.title}</span>
                  </NavLink>
                )
              })}
            </div>
          ))}
        </nav>
      </aside>

      <main className="min-w-0 px-4 py-4 sm:px-6 lg:px-6 lg:py-5">
        <div className="mx-auto w-full max-w-screen-2xl">
          <header className="mb-5 flex min-h-16 flex-col items-start justify-between gap-4 sm:flex-row sm:items-center">
            <div>
              <Breadcrumb>
                <BreadcrumbList>
                  <BreadcrumbItem>
                    <BreadcrumbLink href="/app">
                      {t('common.workspace')}
                    </BreadcrumbLink>
                  </BreadcrumbItem>
                  <BreadcrumbSeparator />
                  <BreadcrumbItem>
                    <BreadcrumbPage>{currentRoute.title}</BreadcrumbPage>
                  </BreadcrumbItem>
                </BreadcrumbList>
              </Breadcrumb>
              <h1 className="mt-1 text-2xl font-bold text-strong-foreground">
                {currentRoute.title}
              </h1>
            </div>
            <div className="flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-center sm:justify-end">
              <span className="rounded-full bg-primary-muted px-3 py-1 text-xs font-bold text-primary-muted-foreground">
                {t('layout.environment')}
              </span>
              <button className="flex h-9 items-center gap-2 rounded-lg border border-border bg-surface px-2.5 text-sm">
                <span className="grid size-6 place-items-center rounded-full bg-info-muted text-xs font-extrabold text-info-muted-foreground">
                  {t('layout.userName').slice(0, 1)}
                </span>
                <span>{t('layout.userName')}</span>
              </button>
            </div>
          </header>

          <Outlet />
        </div>
      </main>
    </div>
  )
}
