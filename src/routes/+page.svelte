<script lang="ts">
  import { goto } from '$app/navigation'
  import { createQuery } from '@tanstack/svelte-query'

  import { isWorkspaceReady } from '@/app/router/routeConfig'
  import { getAppState } from '@/modules/settings'
  import { Spinner } from '@/shared/ui'

  const appState = createQuery(() => ({
    queryKey: ['app-state'],
    queryFn: getAppState,
  }))

  $effect(() => {
    if (!appState.data) return
    void goto(isWorkspaceReady(appState.data) ? '/app/sync' : '/app/onboarding', {
      replaceState: true,
    })
  })
</script>

<div class="flex min-h-screen items-center justify-center bg-background">
  <Spinner class="size-6" />
</div>
