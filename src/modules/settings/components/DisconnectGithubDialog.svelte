<script lang="ts">
  import { t } from '@/shared/i18n'
  import {
    Button,
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
  } from '@/shared/ui'

  interface Props {
    onConfirm: () => void
  }

  let { onConfirm, open = $bindable(false) }: Props & { open?: boolean } = $props()
</script>

<Dialog bind:open>
  <DialogContent>
    <DialogHeader>
      <DialogTitle>{t('settings.disconnectTitle')}</DialogTitle>
      <DialogDescription>{t('settings.disconnectConfirm')}</DialogDescription>
    </DialogHeader>
    <DialogFooter>
      <Button variant="outline" onclick={() => (open = false)}>
        {t('common.actions.cancel')}
      </Button>
      <Button variant="destructive" onclick={onConfirm}>
        {t('settings.disconnectGithub')}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>
