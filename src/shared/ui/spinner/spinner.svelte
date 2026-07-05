<script lang="ts">
  import { Loader2 } from '@lucide/svelte'
  import { cn } from '@/shared/lib/utils'

  let { class: className }: { class?: string } = $props()
</script>

<Loader2 class={cn('size-4 animate-spin text-muted-foreground', className)} />
