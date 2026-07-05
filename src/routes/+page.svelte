<script lang="ts">
  import { goto } from '$app/navigation'
  import { onMount } from 'svelte'
  import { DEFAULT_ROUTE_PATH } from '@/app/router/routeConfig'

  onMount(() => {
    void goto(DEFAULT_ROUTE_PATH, { replaceState: true })
  })
</script>
