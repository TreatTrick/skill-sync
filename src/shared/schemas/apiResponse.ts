import { z } from 'zod'

export const apiSuccessSchema = <TValue extends z.ZodType>(
  dataSchema: TValue,
) =>
  z.object({
    data: dataSchema,
  })
