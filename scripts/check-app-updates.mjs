import assert from 'node:assert/strict'
import { readFile, writeFile, mkdir } from 'node:fs/promises'
import { compileModule } from 'svelte/compiler'
import ts from 'typescript'

const { appUpdateQueryOptions, getSoftwareInfo } =
  await import('../src/modules/about/api/updateApi.ts')
let checkResult = null
let checkError = null
const closedResources = []
globalThis.window = {
  __TAURI_OS_PLUGIN_INTERNALS__: { platform: 'windows', arch: 'x86_64' },
  __TAURI_INTERNALS__: {
    invoke: async (command, args) => {
      if (command === 'plugin:app|name') return 'Skill Sync'
      if (command === 'plugin:app|version') return '1.1.0'
      if (command === 'plugin:updater|check') {
        if (checkError) throw checkError
        return checkResult
      }
      if (command === 'plugin:resources|close') {
        closedResources.push(args.rid)
        return
      }
      throw new Error(`Unexpected native command: ${command}`)
    },
  },
}
assert.deepEqual(await getSoftwareInfo(), {
  name: 'Skill Sync',
  version: '1.1.0',
  platform: 'windows',
  arch: 'x86_64',
})
assert.equal(await appUpdateQueryOptions.queryFn(), null)
checkResult = {
  rid: 10,
  currentVersion: '1.1.0',
  version: '1.2.0',
  date: null,
  body: null,
  rawJson: {},
}
assert.equal(
  (await appUpdateQueryOptions.queryFn()).releaseUrl,
  'https://github.com/TreatTrick/skill-sync/releases/tag/v1.2.0',
)
checkResult = { ...checkResult, rid: 11, version: '1.3.0-beta.1' }
await assert.rejects(appUpdateQueryOptions.queryFn)
assert.deepEqual(
  closedResources,
  [10, 11],
  '旧资源和不合法元数据的资源必须释放',
)
checkResult = null
checkError = new Error('network unavailable')
await assert.rejects(appUpdateQueryOptions.queryFn, /network unavailable/)
checkError = null
assert.equal(
  await appUpdateQueryOptions.queryFn(),
  null,
  '网络错误后仍能重新检查',
)

const source = await readFile(
  'src/modules/about/state/updateInstaller.svelte.ts',
  'utf8',
)
const javascript = ts.transpileModule(source, {
  compilerOptions: {
    target: ts.ScriptTarget.ESNext,
    module: ts.ModuleKind.ESNext,
  },
}).outputText
const compiled = compileModule(javascript, {
  filename: 'updateInstaller.svelte.js',
  generate: 'client',
}).js.code
await mkdir('.svelte-kit', { recursive: true })
await writeFile('.svelte-kit/updateInstaller.test.mjs', compiled)
const { UpdateInstaller } =
  await import('../.svelte-kit/updateInstaller.test.mjs')

let finishDownload
let downloadCalls = 0
let installs = 0
let restartCalls = 0
const installer = new UpdateInstaller(async () => {
  restartCalls += 1
})
const update = {
  download: async (onEvent) => {
    downloadCalls += 1
    onEvent({ event: 'Started', data: { contentLength: 100 } })
    onEvent({ event: 'Progress', data: { chunkLength: 40 } })
    await new Promise((resolve) => (finishDownload = resolve))
    onEvent({ event: 'Finished' })
  },
  install: async () => {
    installs += 1
  },
}
// 原生网络边界使用替身，断言实际安装状态及用户操作门槛。
const downloading = installer.download(update)
assert.equal(installer.status, 'downloading')
assert.equal(installer.downloadedBytes, 40)
assert.equal(installer.totalBytes, 100)
await installer.download(update)
assert.equal(downloadCalls, 1, '重复点击不应启动第二次下载')
finishDownload()
await downloading
assert.equal(installer.status, 'downloaded')
assert.equal(installs, 0, '完成下载不应自动安装')
await installer.install(() => true)
assert.equal(installs, 0, '同步写入期间不应启动安装器')
assert.equal(installer.status, 'downloaded')
assert.equal(installer.errorStage, 'busy')
await installer.install(() => false)
assert.equal(installs, 1)
assert.equal(installer.status, 'installed')
assert.equal(restartCalls, 0, '安装后不应自动重启')
await installer.restart(() => true)
assert.equal(restartCalls, 0, '同步写入期间不应重启')
await installer.restart(() => false)
assert.equal(restartCalls, 1)

const failed = new UpdateInstaller(async () => {
  throw new Error('restart failed')
})
let attempts = 0
const retryable = {
  download: async () => {
    attempts += 1
    if (attempts === 1) throw new Error('invalid signature')
  },
  install: async () => {
    throw new Error('permission denied')
  },
}
await failed.download(retryable)
assert.equal(failed.status, 'idle')
assert.equal(failed.errorStage, 'download')
await failed.install(() => false)
assert.equal(failed.status, 'idle', '校验失败的下载不能安装')
await failed.download(retryable)
assert.equal(failed.status, 'downloaded')
await failed.install(() => false)
assert.equal(failed.status, 'downloaded')
assert.equal(failed.errorStage, 'install')
await failed.restart(() => false)
assert.equal(failed.status, 'downloaded', '安装失败不能请求重启')
retryable.install = async () => {}
await failed.install(() => false)
await failed.restart(() => false)
assert.equal(failed.status, 'installed')
assert.equal(failed.errorStage, 'restart')
console.log('App update checks passed.')
