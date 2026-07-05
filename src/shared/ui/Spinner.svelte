<script lang="ts">
  import { Loader2 } from '@lucide/svelte'
  import { cn } from '@/shared/lib'

  interface Props {
    class?: string
  }

  let { class: className }: Props = $props()
</script>

<Loader2 class={cn('size-4 animate-spin text-muted-foreground', className)} />
