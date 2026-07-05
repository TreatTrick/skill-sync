<script lang="ts">
  import { page } from '$app/state'
  import { PanelLeftClose, PanelLeftOpen } from '@lucide/svelte'

  import { cn } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import { uiState } from '@/shared/state'

  import { appRoutes, type RouteGroupKey } from '../router/routeConfig'

  let { children } = $props()

  const routeGroups = appRoutes.reduce<Record<string, typeof appRoutes>>(
    (groups, route) => {
      groups[route.group] = [...(groups[route.group] ?? []), route]
      return groups
    },
    {},
  )

  const currentRoute = $derived(
    appRoutes.find((route) => route.path === page.url.pathname) ?? appRoutes[0],
  )
</script>

<div
  class={cn(
    'grid min-h-screen grid-cols-1 bg-background text-foreground transition-[grid-template-columns] duration-200',
    uiState.sidebarCollapsed
      ? 'lg:grid-cols-[64px_minmax(0,1fr)]'
      : 'lg:grid-cols-[260px_minmax(0,1fr)]',
  )}
>
  <aside
    class="flex flex-col border-b border-border bg-surface lg:sticky lg:top-0 lg:h-screen lg:border-b-0 lg:border-r"
  >
    <div
      class={cn(
        'flex min-h-16 shrink-0 items-center gap-3 px-4 py-4',
        uiState.sidebarCollapsed && 'lg:justify-center lg:px-0',
      )}
    >
      <img alt="" class="size-9 shrink-0 rounded-lg shadow-sm" src="/favicon.svg" />
      <div class={cn('min-w-0', uiState.sidebarCollapsed && 'lg:hidden')}>
        <div class="truncate text-base font-extrabold text-strong-foreground">
          {t('layout.brandTitle')}
        </div>
        <div class="mt-0.5 truncate text-xs text-muted-foreground">
          {t('layout.brandSubtitle')}
        </div>
      </div>
    </div>

    <nav
      aria-label={t('layout.navLabel')}
      class="grid flex-1 content-start gap-4 px-3 pb-4 pt-2 sm:grid-cols-2 lg:grid-cols-1 lg:overflow-auto"
    >
      {#each Object.entries(routeGroups) as [group, routes] (group)}
        <div class="grid gap-1">
          <div
            class={cn(
              'px-2.5 pb-1 text-xs font-bold uppercase tracking-wide text-muted-foreground',
              uiState.sidebarCollapsed && 'lg:hidden',
            )}
          >
            {t(group as RouteGroupKey)}
          </div>
          {#each routes as route (route.path)}
            {@const Icon = route.icon}
            {@const isActive = page.url.pathname === route.path}
            <a
              class={cn(
                'flex h-9 items-center gap-2.5 rounded-lg px-2.5 text-sm font-medium transition-colors',
                uiState.sidebarCollapsed && 'lg:justify-center lg:px-0',
                isActive
                  ? 'bg-primary-muted font-bold text-primary-muted-foreground'
                  : 'text-foreground hover:bg-surface-hover',
              )}
              href={route.path}
              title={uiState.sidebarCollapsed ? t(route.title) : undefined}
            >
              <Icon class="size-4 shrink-0" />
              <span class={cn('truncate', uiState.sidebarCollapsed && 'lg:hidden')}>
                {t(route.title)}
              </span>
            </a>
          {/each}
        </div>
      {/each}
    </nav>

    <div class="hidden shrink-0 px-3 pb-4 pt-2 lg:block">
      <button
        aria-label={uiState.sidebarCollapsed
          ? t('layout.expand')
          : t('layout.collapse')}
        class={cn(
          'flex h-9 w-full items-center gap-2.5 rounded-lg px-2.5 text-sm font-medium text-muted-foreground transition-colors hover:bg-surface-hover',
          uiState.sidebarCollapsed && 'lg:justify-center lg:px-0',
        )}
        onclick={() => uiState.setSidebarCollapsed(!uiState.sidebarCollapsed)}
        title={uiState.sidebarCollapsed ? t('layout.expand') : t('layout.collapse')}
        type="button"
      >
        {#if uiState.sidebarCollapsed}
          <PanelLeftOpen class="size-4 shrink-0" />
        {:else}
          <PanelLeftClose class="size-4 shrink-0" />
        {/if}
        {#if !uiState.sidebarCollapsed}
          <span class="truncate">{t('layout.collapse')}</span>
        {/if}
      </button>
    </div>
  </aside>

  <main class="min-w-0">
    <header
      class="sticky top-0 z-10 border-b border-border bg-background/80 backdrop-blur"
    >
      <div class="mx-auto w-full max-w-screen-2xl px-4 py-4 sm:px-6">
        <nav
          aria-label="breadcrumb"
          class="flex items-center gap-1.5 text-sm text-muted-foreground"
        >
          <a class="hover:text-foreground" href="/app">{t('common.workspace')}</a>
          <span aria-hidden="true">/</span>
          <span class="text-foreground">{t(currentRoute.title)}</span>
        </nav>
        <h1 class="mt-1.5 text-2xl font-bold text-strong-foreground">
          {t(currentRoute.title)}
        </h1>
      </div>
    </header>

    <div class="mx-auto w-full max-w-screen-2xl px-4 py-5 sm:px-6">
      {@render children?.()}
    </div>
  </main>
</div>
