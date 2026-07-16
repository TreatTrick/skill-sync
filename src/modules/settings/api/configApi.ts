import { invokeCmd } from '@/shared/lib'
import { appConfigSchema, type AppConfig } from '@/shared/schemas'

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
