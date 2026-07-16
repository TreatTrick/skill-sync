import { invokeCmd } from './tauri'
import { scanResultSchema, type ScanResult } from '@/shared/schemas'

export const scanSkills = async (): Promise<ScanResult> => {
  const raw = await invokeCmd<unknown>('scan_skills')
  return scanResultSchema.parse(raw)
}
