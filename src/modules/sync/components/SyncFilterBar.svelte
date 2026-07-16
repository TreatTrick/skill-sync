<script lang="ts">
  import { t } from '@/shared/i18n'
  import { Button, Input } from '@/shared/ui'

  interface Props {
    search: string
    canSelectAll: boolean
    canSelectNone: boolean
    onSelectAll: () => void
    onSelectNone: () => void
  }

  let {
    search = $bindable(),
    canSelectAll,
    canSelectNone,
    onSelectAll,
    onSelectNone,
  }: Props = $props()
</script>

<div class="flex flex-col gap-3 border-b border-border pb-4 sm:flex-row">
  <Input bind:value={search} placeholder={t('sync.searchPlaceholder')} />
  <div class="flex gap-2">
    <Button
      variant="outline"
      class="flex-1 sm:flex-none"
      onclick={onSelectAll}
      disabled={!canSelectAll}
    >
      {t('sync.selectAll')}
    </Button>
    <Button
      variant="outline"
      class="flex-1 sm:flex-none"
      onclick={onSelectNone}
      disabled={!canSelectNone}
    >
      {t('sync.selectNone')}
    </Button>
  </div>
</div>
