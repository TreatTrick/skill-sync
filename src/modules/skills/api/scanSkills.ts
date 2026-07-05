import { invokeCmd } from '@/shared/lib'

import { scanResultSchema, type ScanResult } from '../schemas/skill'

export const scanSkills = async (): Promise<ScanResult> => {
  const raw = await invokeCmd<unknown>('scan_skills')
  return scanResultSchema.parse(raw)
}
