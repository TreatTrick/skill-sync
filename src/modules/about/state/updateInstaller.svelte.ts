import { relaunch } from '@tauri-apps/plugin-process'
import type { Update } from '@tauri-apps/plugin-updater'

type InstallStatus =
  | 'idle'
  | 'downloading'
  | 'downloaded'
  | 'installing'
  | 'installed'
  | 'restarting'

export class UpdateInstaller {
  status = $state<InstallStatus>('idle')
  downloadedBytes = $state(0)
  totalBytes = $state<number | undefined>()
  error = $state<unknown>(null)
  errorStage = $state<'download' | 'install' | 'restart' | 'busy' | null>(null)
  private resource: Update | null = null
  private restartApp: () => Promise<void>

  constructor(restartApp = relaunch) {
    this.restartApp = restartApp
  }

  async download(resource: Update): Promise<void> {
    if (this.status !== 'idle') return
    this.status = 'downloading'
    this.error = null
    this.errorStage = null
    this.downloadedBytes = 0
    this.totalBytes = undefined
    this.resource = resource
    try {
      await resource.download(
        (event) => {
          if (event.event === 'Started') {
            this.totalBytes = event.data.contentLength
          } else if (event.event === 'Progress') {
            this.downloadedBytes += event.data.chunkLength
          }
        },
        { timeout: 300_000 },
      )
      this.status = 'downloaded'
    } catch (error) {
      this.error = error
      this.errorStage = 'download'
      this.status = 'idle'
    }
  }

  async install(hasPendingWork: () => boolean): Promise<void> {
    if (this.status !== 'downloaded' || !this.resource) return
    if (hasPendingWork()) {
      this.errorStage = 'busy'
      return
    }
    this.status = 'installing'
    this.error = null
    this.errorStage = null
    try {
      await this.resource.install()
      this.status = 'installed'
    } catch (error) {
      this.error = error
      this.errorStage = 'install'
      this.status = 'downloaded'
    }
  }

  async restart(hasPendingWork: () => boolean): Promise<void> {
    if (this.status !== 'installed') return
    if (hasPendingWork()) {
      this.errorStage = 'busy'
      return
    }
    this.status = 'restarting'
    this.error = null
    this.errorStage = null
    try {
      await this.restartApp()
    } catch (error) {
      this.error = error
      this.errorStage = 'restart'
      this.status = 'installed'
    }
  }
}

export const updateInstaller = new UpdateInstaller()
