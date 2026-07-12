<script lang="ts">
  import { page } from '$app/state'
  import { goto } from '$app/navigation'
  import { createQuery } from '@tanstack/svelte-query'

  import AppLayout from '@/app/layouts/AppLayout.svelte'
  import { isWorkspaceReady } from '@/app/router/routeConfig'
  import { getAppState } from '@/modules/settings'
  import { errorMessage } from '@/shared/lib'
  import { Card, CardContent, Spinner } from '@/shared/ui'

  let { children } = $props()
  const appState = createQuery(() => ({
    queryKey: ['app-state'],
    queryFn: getAppState,
  }))
  const isOnboarding = $derived(page.url.pathname === '/app/onboarding')
  const isReconfigure = $derived(page.url.searchParams.get('mode') === 'reconfigure')
  const ready = $derived(appState.data ? isWorkspaceReady(appState.data) : false)

  $effect(() => {
    if (!appState.data) return
    if (isOnboarding) {
      if (ready && !isReconfigure) void goto('/app/sync', { replaceState: true })
    } else if (!ready) {
      void goto('/app/onboarding', { replaceState: true })
    }
  })
</script>

{#if appState.isLoading}
  <div class="flex min-h-screen items-center justify-center bg-background">
    <Spinner class="size-6" />
  </div>
{:else if appState.error}
  <div class="flex min-h-screen items-center justify-center bg-background p-4">
    <Card class="w-full max-w-md border-destructive-border bg-destructive-muted">
      <CardContent class="pt-6 text-sm text-destructive">
        {errorMessage(appState.error)}
      </CardContent>
    </Card>
  </div>
{:else if isOnboarding}
  {@render children?.()}
{:else if ready}
  <AppLayout>{@render children?.()}</AppLayout>
{:else}
  <div class="flex min-h-screen items-center justify-center bg-background">
    <Spinner class="size-6" />
  </div>
{/if}
