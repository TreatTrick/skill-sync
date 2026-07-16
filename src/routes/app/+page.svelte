<script lang="ts">
  import { goto } from '$app/navigation'
  import { onMount } from 'svelte'

  import { Spinner } from '@/shared/ui'

  // The app layout has already settled the workspace-ready guard before this
  // index renders, so it is safe to forward to the default sync workspace.
  onMount(() => {
    void goto('/app/sync', { replaceState: true })
  })
</script>

<div class="flex min-h-screen items-center justify-center bg-background">
  <Spinner class="size-6" />
</div>
