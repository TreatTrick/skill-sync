<script lang="ts">
  import { Copy, ExternalLink } from '@lucide/svelte'

  import { t } from '@/shared/i18n'
  import type { scanSkills } from '@/modules/skills'
  import { Button, Card, CardContent, Skeleton } from '@/shared/ui'

  type SkillRoot = Awaited<ReturnType<typeof scanSkills>>['roots'][number]

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

<div class="grid gap-3">
  <div>
    <h2 class="text-lg font-semibold text-strong-foreground">{t('settings.skillRoots')}</h2>
    <p class="text-sm text-muted-foreground">{t('settings.skillRootsReadOnly')}</p>
  </div>
  {#if isLoading}
    <div class="grid gap-3 lg:grid-cols-3">
      {#each Array(3) as _, i (i)}
        <div class="rounded-xl border border-border bg-card p-4">
          <Skeleton class="h-5 w-32" />
          <Skeleton class="mt-3 h-3 w-full" />
        </div>
      {/each}
    </div>
  {:else}
    <div class="grid gap-3 lg:grid-cols-3">
      {#each roots as root (root.namespace)}
        <Card class="transition-shadow hover:shadow-md">
          <CardContent class="grid gap-3 p-4">
            <div class="flex items-start justify-between gap-2">
              <div>
                <h3 class="font-semibold text-strong-foreground">{t(namespaceLabelKeys[root.namespace])}</h3>
                <p class="mt-1 break-all text-xs text-muted-foreground">{root.root_path}</p>
              </div>
              <span class="text-xs {root.exists && root.readable ? 'text-success' : 'text-warning'}">
                {root.exists ? (root.readable ? t('common.status.ready') : t('settings.rootUnreadable')) : t('settings.rootNotFound')}
              </span>
            </div>
            <div class="flex gap-2">
              <Button onclick={() => onOpenPath(root.root_path)} size="sm" variant="outline">
                <ExternalLink class="size-4" />{t('common.actions.open')}
              </Button>
              <Button onclick={() => onCopyPath(root.root_path)} size="sm" variant="ghost">
                <Copy class="size-4" />{t('settings.copyPath')}
              </Button>
            </div>
          </CardContent>
        </Card>
      {/each}
    </div>
  {/if}
</div>
