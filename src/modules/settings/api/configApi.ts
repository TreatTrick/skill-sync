import { invokeCmd } from '@/shared/lib'
import {
  appConfigSchema,
  cacheStatsSchema,
  type AppConfig,
  type CacheStats,
} from '@/shared/schemas'

export const saveConfig = async (config: AppConfig): Promise<void> => {
  const parsed = appConfigSchema.parse(config)
  await invokeCmd('save_config', { config: parsed })
}

export const disconnectGithub = async (
  expectedRepositoryId: number,
): Promise<void> => {
  await invokeCmd('disconnect_github', {
    expectedRepositoryId,
  })
}

export const getCacheStats = async (): Promise<CacheStats> =>
  cacheStatsSchema.parse(await invokeCmd<unknown>('get_cache_stats'))

export const clearSkillPackCache = async (): Promise<void> => {
  await invokeCmd('clear_skill_pack_cache')
}
