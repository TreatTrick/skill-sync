import { invokeCmd } from './tauri'
import { appStateSchema, type AppState } from '@/shared/schemas'

export const getAppState = async (): Promise<AppState> => {
  const raw = await invokeCmd<unknown>('get_app_state')
  return appStateSchema.parse(raw)
}
