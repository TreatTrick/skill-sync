<script lang="ts">
  import { goto } from '$app/navigation'
  import { ArrowDownToLine, ArrowUpFromLine, Ban, Check, ChevronDown, Copy, FolderOpen, Info, Trash2 } from '@lucide/svelte'
  import { t } from '@/shared/i18n'
  import { cn, errorMessage } from '@/shared/lib'
  import { Button, Checkbox, toast } from '@/shared/ui'
  import type { SyncDecision, SyncSkillEntry } from '../schemas/syncPlan'
  import { decisionLabelKey } from '../lib/syncStatus'

  interface Props {
    entry: SyncSkillEntry
    decision?: SyncDecision
    decisionOptions?: readonly SyncDecision[]
    selected?: boolean
    onToggle?: (selected: boolean) => void
    onDecision?: (choice: SyncDecision | '') => void
    onOpenFolder?: () => void
  }
  let { entry, decision, decisionOptions = [], selected = false, onToggle, onDecision, onOpenFolder }: Props = $props()
  let detailsOpen = $state(false)
  const isDelete = $derived(entry.status === 'local_deleted' || entry.status === 'remote_deleted')
  const isSimple = $derived(entry.status === 'local_update' || entry.status === 'remote_update')
  const isSynced = $derived(entry.status === 'synced' || entry.status === 'both_deleted')
  const isBlocked = $derived(entry.status === 'blocked' || entry.status === 'unknown')
  const selectedDecision = $derived(decision ?? (isDelete && selected ? (entry.status === 'local_deleted' ? 'delete_remote' : 'accept_delete') : ''))
  const outcome = $derived(
    selectedDecision === 'keep_local' || (entry.status === 'local_update' && selected) ? 'up'
      : selectedDecision === 'use_remote' || selectedDecision === 'restore_remote' || (entry.status === 'remote_update' && selected) ? 'down'
      : selectedDecision === 'delete_remote' ? 'delRemote'
      : selectedDecision === 'accept_delete' ? 'delLocal'
      : null,
  )
  const reason = $derived(
    entry.conflict_reason ? t(`sync.conflictReason.${entry.conflict_reason}`)
      : entry.status === 'local_deleted' ? t('sync.localDeletedHint')
      : entry.status === 'remote_deleted' ? t('sync.remoteDeletedHint')
      : entry.status === 'local_update' ? t(entry.remote_hash ? 'sync.localChanged' : 'sync.newLocal')
      : entry.status === 'remote_update' ? t(entry.local_hash ? 'sync.remoteChanged' : 'sync.noLocalYet')
      : '',
  )
  const namespaceLabels = { agents: 'sync.namespace.agents', codex: 'sync.namespace.codex', 'claude-code': 'sync.namespace.claudeCode' } as const
  const hashState = (hash: string | null, baseline = false) => {
    if (baseline) return { tone: hash ? 'same' : 'none', label: t(hash ? 'sync.hashBaseline' : 'sync.neverSynced') }
    if (!hash) return { tone: entry.base_hash ? 'gone' : 'none', label: t(entry.base_hash ? 'sync.hashDeleted' : 'sync.noHash') }
    return hash === entry.base_hash
      ? { tone: 'same', label: t('sync.hashUnchanged') }
      : { tone: 'chg', label: t(entry.base_hash ? 'sync.hashChanged' : 'sync.newEntry') }
  }
  const copyPath = async (): Promise<void> => {
    if (!entry.local_path) return
    try { await navigator.clipboard.writeText(entry.local_path); toast.success(t('settings.pathCopied')) }
    catch (error) { toast.error(errorMessage(error)) }
  }
</script>

<article class={cn('row', !outcome && !isSynced && !isBlocked && !(entry.status === 'conflict' && !decision) && 'dim')}>
  <div class="row-main">
    <span class="lead">
      {#if isSimple}
        <Checkbox class="row-check" aria-label={t('sync.selectAction', { name: entry.name })} checked={selected} onCheckedChange={(checked) => onToggle?.(checked === true)} />
      {:else if outcome}<span class="ok"><Check class="ic" /></span>
      {:else if entry.status === 'conflict' && !decision}<span class="pend" title={t('sync.undecidedConflict')}></span>
      {:else if isBlocked}<Ban class="ic text-destructive" />
      {:else if isSynced}<Check class="ic text-success" />
      {:else}<span class="skip" title={t('sync.skip')}></span>{/if}
    </span>
    <div class="ident">
      <span class="name" title={entry.skill_id}>{entry.name}</span><span class="ns">{t(namespaceLabels[entry.namespace])}</span>
      {#if isSimple && !entry.base_hash && (!entry.local_hash || !entry.remote_hash)}<span class="newtag">{t('sync.newEntry')}</span>{/if}
      {#if entry.warnings.length}<span class="flex text-warning" title={t('sync.entry.warning')}><Info class="ic sm" /></span>{/if}
    </div>
    {#if outcome}
      <div class={cn('what', outcome === 'up' ? 'up' : outcome === 'down' ? 'down' : 'del')}>
        {#if outcome === 'up'}<ArrowUpFromLine class="ic" />{:else if outcome === 'down'}<ArrowDownToLine class="ic" />{:else}<Trash2 class="ic" />{/if}
        <span>
          {#if isSimple}{t(outcome === 'up' ? 'sync.uploadAction' : 'sync.downloadAction')}
          {:else if outcome === 'up'}{t(entry.status === 'remote_deleted' ? 'sync.reupload' : 'sync.outcome.keepLocal')}
          {:else if outcome === 'down'}{t(entry.local_hash ? 'sync.outcome.useRemote' : 'sync.redownload')}
          {:else}{t(outcome === 'delRemote' ? 'sync.outcome.deleteRemote' : 'sync.outcome.acceptDelete')}{/if}
          <span class="because">· {reason}</span>
        </span>
      </div>
    {:else if isBlocked}<div class="what del"><span>{entry.blocked_reason ?? t('sync.status.unknown')}</span></div>
    {:else if isSynced}<div class="what off"><span class="mono synced-path">{entry.local_path ?? entry.relative_dir ?? t('sync.notAvailable')}</span></div>
    {:else}<div class:warn={entry.status === 'conflict' && !decision} class:off={entry.status !== 'conflict' || !!decision} class="what"><span>{isSimple ? t('sync.skip') + ' · ' : ''}{reason}{selectedDecision === 'skip' && !isDelete ? ' · ' + t('sync.skip') : ''}</span></div>{/if}
    <div class="act">
      {#if decisionOptions.length}
        <div class="seg" role="group" aria-label={t('sync.decisionFor', { name: entry.name })}>
          {#each decisionOptions as choice (choice)}
            <button type="button" aria-pressed={choice === 'skip' && isDelete ? !selectedDecision || selectedDecision === 'skip' : selectedDecision === choice}
              data-tone={choice === 'keep_local' ? 'up' : choice === 'use_remote' || choice === 'restore_remote' ? 'down' : choice === 'delete_remote' || choice === 'accept_delete' ? 'del' : undefined}
              onclick={() => onDecision?.(isDelete && choice === 'skip' ? '' : selectedDecision === choice ? '' : choice)}>
              {choice === 'skip' && isDelete ? t('sync.deferDelete') : choice === 'keep_local' && entry.status === 'remote_deleted' ? t('sync.reuploadShort') : t(decisionLabelKey(choice))}
            </button>
          {/each}
        </div>
      {:else if isBlocked}<Button class="btn sm" onclick={() => void goto('/app/settings#settings-limits')} variant="outline">{t('settings.adjustLimits')}</Button>{/if}
    </div>
    <button type="button" aria-expanded={detailsOpen} aria-label={t(detailsOpen ? 'sync.collapseDetails' : 'sync.expandDetails', { name: entry.name })} class="more" onclick={() => (detailsOpen = !detailsOpen)}><ChevronDown class="ic" /></button>
  </div>
  {#if detailsOpen}
    <div class="detail">
      <div><h3>{t('sync.detailTitle')}</h3><dl class="kv">
        <dt>{t('sync.entry.skillId')}</dt><dd><span class="mono">{entry.skill_id}</span></dd>
        <dt>{t('sync.localPath')}</dt><dd>
          {#if entry.local_path}<span class="mono" title={entry.local_path}>{entry.local_path}</span>
            {#if onOpenFolder}<Button class="btn ghost sm icon" aria-label={t('skills.openFolder')} title={t('skills.openFolder')} onclick={onOpenFolder} variant="ghost"><FolderOpen class="ic sm" /></Button>{/if}
            <Button class="btn ghost sm icon" aria-label={t('settings.copyPath')} title={t('settings.copyPath')} onclick={() => void copyPath()} variant="ghost"><Copy class="ic sm" /></Button>
          {:else}<span class="muted">{t('sync.noLocalYet')}</span>{/if}
        </dd>
      </dl></div>
      <div><h3>{t('sync.hashTitle')}</h3><div class="tri">
        {#each [{ label: 'sync.hashLabels.base', value: entry.base_hash, baseline: true }, { label: 'sync.hashLabels.local', value: entry.local_hash, baseline: false }, { label: 'sync.hashLabels.remote', value: entry.remote_hash, baseline: false }] as hash (hash.label)}
          {@const state = hashState(hash.value, hash.baseline)}
          <div class={state.tone}><small>{t(hash.label as 'sync.hashLabels.base' | 'sync.hashLabels.local' | 'sync.hashLabels.remote')}<i>{state.label}</i></small><code title={hash.value ?? undefined}>{hash.value?.slice(0, 12) ?? '—'}</code></div>
        {/each}
      </div></div>
      {#if entry.warnings.length}<ul class="warns">{#each entry.warnings as warning (warning)}<li>{warning}</li>{/each}</ul>{/if}
    </div>
  {/if}
</article>
