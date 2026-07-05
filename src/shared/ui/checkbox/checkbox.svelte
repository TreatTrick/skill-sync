<script lang="ts">
  import { Checkbox as CheckboxPrimitive } from 'bits-ui'
  import { Check } from '@lucide/svelte'
  import { cn } from '@/shared/lib/utils'

  let {
    class: className,
    checked = $bindable(),
    ...restProps
  }: CheckboxPrimitive.RootProps = $props()
</script>

<CheckboxPrimitive.Root
  bind:checked
  class={cn(
    'group peer box-content size-4 shrink-0 rounded-sm border border-primary shadow focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-primary data-[state=checked]:text-primary-foreground',
    className,
  )}
  {...restProps}
>
  <Check
    class="size-3.5 opacity-0 transition-opacity group-data-[state=checked]:opacity-100"
  />
</CheckboxPrimitive.Root>
