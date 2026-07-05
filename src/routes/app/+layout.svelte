<script lang="ts">
  import AppLayout from '@/app/layouts/AppLayout.svelte'

  let { children } = $props()
</script>

<AppLayout>
  {@render children?.()}
</AppLayout>
