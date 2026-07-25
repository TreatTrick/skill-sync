<script lang="ts">
  import { Download } from '@lucide/svelte'
  import { i18n } from '../i18n.svelte'
  import { DOWNLOADS, RELEASES_URL } from '../links'
</script>

<section class="border-t border-border bg-surface">
  <div class="mx-auto max-w-4xl px-4 py-16 sm:px-6 sm:py-20">
    <h2
      class="text-center text-3xl font-bold tracking-tight text-strong-foreground"
    >
      {i18n.t('download.title')}
    </h2>
    <p class="mx-auto mt-4 max-w-2xl text-center text-muted-foreground">
      {i18n.t('download.subtitle')}
    </p>
    <div
      class="mt-10 overflow-hidden rounded-xl border border-border bg-background"
    >
      <table class="w-full text-left text-sm">
        <thead
          class="border-b border-border bg-surface-muted text-muted-foreground"
        >
          <tr>
            <th class="px-4 py-3 font-medium">
              {i18n.t('download.platform')}
            </th>
            <th class="px-4 py-3 font-medium">
              {i18n.t('download.type')}
            </th>
            <th class="px-4 py-3 text-right font-medium">
              {i18n.t('download.file')}
            </th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border">
          {#each DOWNLOADS as item (item.url)}
            <tr class="transition-colors hover:bg-surface-hover">
              <td class="px-4 py-3 font-medium text-foreground">
                {i18n.t(item.platformKey)}
              </td>
              <td class="px-4 py-3 text-muted-foreground">
                {i18n.t(item.typeKey)}
              </td>
              <td class="px-4 py-3 text-right">
                <a
                  href={item.url}
                  class="inline-flex items-center gap-1.5 font-medium text-primary hover:underline"
                >
                  <Download class="h-4 w-4" />
                  {i18n.t('download.file')}
                </a>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <p class="mt-4 text-center text-xs text-muted-foreground">
      {i18n.t('download.note')}
    </p>
    <div class="mt-6 text-center">
      <a
        href={RELEASES_URL}
        class="inline-flex h-10 items-center justify-center rounded-md border border-border bg-background px-5 text-sm font-medium text-foreground transition-colors hover:bg-surface-hover"
      >
        {i18n.t('download.more')}
      </a>
    </div>
  </div>
</section>
