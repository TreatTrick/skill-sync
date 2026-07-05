<script lang="ts">
  import { FolderOpen } from '@lucide/svelte'
  import { chooseDirectory, cn } from '@/shared/lib'
  import { t } from '@/shared/i18n'

  import Button from './Button.svelte'
  import Input from './Input.svelte'

  interface Props {
    value: string
    onChange: (path: string) => void
    placeholder: string
    class?: string
  }

  let { value, onChange, placeholder, class: className }: Props = $props()

  // Stable unique id for the label/input association; this component is
  // client-only (ssr = false) so the Web Crypto API is always available.
  const inputId = `path-picker-${crypto.randomUUID()}`

  const handlePick = async () => {
    const path = await chooseDirectory()
    if (path) {
      onChange(path)
    }
  }
</script>

<div class={cn('grid gap-1.5', className)}>
  <label class="text-sm font-medium text-muted-foreground" for={inputId}>
    {placeholder}
  </label>
  <div class="flex items-center gap-2">
    <Input
      id={inputId}
      oninput={(event: Event) =>
        onChange((event.currentTarget as HTMLInputElement).value)
      }
      placeholder={placeholder}
      value={value}
    />
    <Button onclick={() => void handlePick()} type="button" variant="secondary">
      {#snippet icon()}
        <FolderOpen class="size-4" />
      {/snippet}
      {t('common.actions.browse')}
    </Button>
  </div>
</div>
