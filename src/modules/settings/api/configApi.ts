import { invokeCmd } from '@/shared/lib'

import {
  appConfigSchema,
  appStateSchema,
  type AppConfig,
  type AppState,
} from '../schemas/config'

export const getAppState = async (): Promise<AppState> => {
  const raw = await invokeCmd<unknown>('get_app_state')
  return appStateSchema.parse(raw)
}

export const saveConfig = async (config: AppConfig): Promise<void> => {
  const parsed = appConfigSchema.parse(config)
  await invokeCmd('save_config', { config: parsed })
}
