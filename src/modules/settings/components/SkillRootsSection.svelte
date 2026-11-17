<script lang="ts">
  import { Copy, ExternalLink } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import type { ScanResult } from '@/shared/schemas'
  import { Button, Skeleton } from '@/shared/ui'

  type SkillRoot = ScanResult['roots'][number]

  interface Props {
    isLoading: boolean
    roots: SkillRoot[]
    onOpenPath: (path: string) => void
    onCopyPath: (path: string) => void
  }

  let { isLoading, roots, onOpenPath, onCopyPath }: Props = $props()

  const namespaceLabelKeys = {
    agents: 'settings.namespace.agents',
    codex: 'settings.namespace.codex',
    'claude-code': 'settings.namespace.claudeCode',
  } as const
</script>
<div class="srows">
  {#if isLoading}
    {#each Array(3) as _, i (i)}<div class="srow"><Skeleton class="h-5 w-32" /></div>{/each}
  {:else}
    {#each roots as root (root.namespace)}
      <div class="srow root">
        <div class="lab"><b>{t(namespaceLabelKeys[root.namespace])}</b><span class="path">{root.root_path}</span></div>
        <div class="ctl">
          <span class="tag" class:ok={root.exists && root.readable}>
            {root.exists ? (root.readable ? t('common.status.ready') : t('settings.rootUnreadable')) : t('settings.rootNotFound')}
          </span>
          <Button class="btn sm" onclick={() => onOpenPath(root.root_path)} variant="outline"><ExternalLink class="size-3.5" />{t('common.actions.open')}</Button>
          <Button aria-label={t('settings.copyPath')} title={t('settings.copyPath')} class="btn sm ghost icon" onclick={() => onCopyPath(root.root_path)} variant="ghost"><Copy class="size-3.5" /></Button>
        </div>
      </div>
    {/each}
  {/if}
</div>
