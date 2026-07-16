<script lang="ts">
  import { goto } from '$app/navigation'
  import { onMount } from 'svelte'

  import { Spinner } from '@/shared/ui'

  // Root only bounces into the app shell; app/+layout.svelte owns the
  // workspace-ready guard (spinner / error / onboarding redirect).
  onMount(() => {
    void goto('/app', { replaceState: true })
  })
</script>

<div class="flex min-h-screen items-center justify-center bg-background">
  <Spinner class="size-6" />
</div>
