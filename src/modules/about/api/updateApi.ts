import { getName, getVersion } from '@tauri-apps/api/app'
import { arch, platform } from '@tauri-apps/plugin-os'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { z } from 'zod'

export const PROJECT_URL = 'https://github.com/TreatTrick/skill-sync'

const softwareInfoSchema = z.object({
  name: z.string().min(1),
  version: z.string().min(1),
  platform: z.string().min(1),
  arch: z.string().min(1),
})

const updateMetadataSchema = z.object({
  currentVersion: z.string().min(1),
  version: z.string().regex(/^\d+\.\d+\.\d+(?:\+[\w.-]+)?$/),
  date: z.iso.datetime({ offset: true }).nullish(),
  body: z.string().nullish(),
})

export const getSoftwareInfo = async () => {
  const [name, version] = await Promise.all([getName(), getVersion()])
  return softwareInfoSchema.parse({
    name,
    version,
    platform: platform(),
    arch: arch(),
  })
}

let previousUpdate: Update | null = null

const checkForAppUpdate = async () => {
  if (previousUpdate) {
    const resource = previousUpdate
    previousUpdate = null
    await resource.close()
  }
  const resource = await check({ timeout: 15_000 })
  if (!resource) return null
  previousUpdate = resource
  try {
    const metadata = updateMetadataSchema.parse(resource)
    return {
      ...metadata,
      releaseUrl: `${PROJECT_URL}/releases/tag/v${encodeURIComponent(metadata.version)}`,
      resource,
    }
  } catch (error) {
    previousUpdate = null
    await resource.close()
    throw error
  }
}

export const appUpdateQueryOptions = {
  queryKey: ['app-update'],
  queryFn: checkForAppUpdate,
  staleTime: Infinity,
  gcTime: Infinity,
  retry: false,
  retryOnMount: false,
  refetchOnMount: false,
  refetchOnWindowFocus: false,
  refetchOnReconnect: false,
} as const
