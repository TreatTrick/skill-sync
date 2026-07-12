<script lang="ts">
  import { goto } from '$app/navigation'
  import { createQuery } from '@tanstack/svelte-query'

  import { isWorkspaceReady } from '@/app/router/routeConfig'
  import { getAppState } from '@/modules/settings'
  import { errorMessage } from '@/shared/lib'
  import { t } from '@/shared/i18n'
  import { Button, Card, CardContent, Spinner } from '@/shared/ui'

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

{#if appState.isLoading}
  <div class="flex min-h-screen items-center justify-center bg-background">
    <Spinner class="size-6" />
  </div>
{:else if appState.error}
  <div class="flex min-h-screen items-center justify-center bg-background p-4">
    <Card class="w-full max-w-md border-destructive-border bg-destructive-muted">
      <CardContent class="grid gap-4 pt-6">
        <p class="text-sm text-destructive">
          {t('common.appStateLoadError', { message: errorMessage(appState.error) })}
        </p>
        <Button onclick={() => void appState.refetch()} size="sm" variant="outline">
          {t('common.actions.retry')}
        </Button>
      </CardContent>
    </Card>
  </div>
{:else}
  <div class="flex min-h-screen items-center justify-center bg-background">
    <Spinner class="size-6" />
  </div>
{/if}
