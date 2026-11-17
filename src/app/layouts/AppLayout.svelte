<script lang="ts">
  import type { Snippet } from 'svelte'
  import { page } from '$app/state'
  import { PanelLeftClose, PanelLeftOpen } from '@lucide/svelte'

  import { cn } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import type { RemoteConfig } from '@/shared/schemas'
  import { uiState } from '@/shared/state'

  import { appRoutes } from '../router/routeConfig'

  interface Props {
    children?: Snippet
    remoteConfig: RemoteConfig | null
  }

  let { children, remoteConfig }: Props = $props()

</script>

<div
  class={cn(
    'prototype-ui grid h-dvh min-h-0 grid-cols-[64px_minmax(0,1fr)] overflow-hidden bg-background text-foreground transition-[grid-template-columns] duration-200',
    uiState.sidebarCollapsed
      ? 'lg:grid-cols-[64px_minmax(0,1fr)]'
      : 'lg:grid-cols-[176px_minmax(0,1fr)]',
  )}
>
  <aside
    class="flex min-h-0 flex-col border-r border-border bg-surface"
  >
    <div
      class={cn(
        'flex min-h-16 shrink-0 items-center justify-center px-2 py-4 lg:justify-start lg:px-4',
        uiState.sidebarCollapsed && 'lg:justify-center lg:px-0',
      )}
    >
      <img alt={t('layout.brandTitle')} class="size-7 shrink-0 rounded-lg shadow-sm" src="/favicon.svg" />
      <div class={cn('hidden min-w-0 lg:block', uiState.sidebarCollapsed && 'lg:hidden')}>
        <div class="truncate text-base font-extrabold text-strong-foreground">
          {t('layout.brandTitle')}
        </div>
      </div>
    </div>

    <nav
      aria-label={t('layout.navLabel')}
      class="grid flex-1 content-start gap-1 overflow-y-auto px-2 pb-4 pt-2 lg:px-3"
    >
      <div class="grid gap-1">
        {#each appRoutes as route (route.path)}
          {@const Icon = route.icon}
          {@const isActive = page.url.pathname === route.path}
          <a
            class={cn(
              'flex min-h-10 items-center justify-center gap-2.5 rounded-lg text-sm font-medium transition-colors lg:justify-start lg:px-2.5',
              uiState.sidebarCollapsed && 'lg:justify-center lg:px-0',
              isActive
                ? 'bg-primary-muted font-bold text-primary-muted-foreground'
                : 'text-foreground hover:bg-surface-hover',
            )}
            href={route.path}
            aria-current={isActive ? 'page' : undefined}
            title={t(route.title)}
          >
            <Icon class="size-4 shrink-0" />
            <span class={cn('hidden truncate lg:block', uiState.sidebarCollapsed && 'lg:hidden')}>
              {t(route.title)}
            </span>
          </a>
        {/each}
      </div>
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

  <main class="workspace-main flex min-h-0 min-w-0 flex-col">
    <header
      class="workspace-top flex h-13 shrink-0 items-center border-b border-border bg-surface px-4 sm:px-6"
    >
      <div class="mx-auto flex w-full min-w-0 items-center justify-end">
          {#if remoteConfig}
            <a
              aria-label={t('layout.vaultContext')}
              class="vault"
              href="/app/settings#settings-vault"
              title={`${remoteConfig.owner}/${remoteConfig.repo} · ${remoteConfig.branch}`}
            >
              <span class="led"></span>
              <span class="repo">
                {remoteConfig.owner}/{remoteConfig.repo}
              </span>
              <span class="br">
                {remoteConfig.branch}
              </span>
            </a>
          {/if}
      </div>
    </header>

    <div class="flex min-h-0 min-w-0 flex-1 flex-col">
      {@render children?.()}
    </div>
  </main>
</div>
