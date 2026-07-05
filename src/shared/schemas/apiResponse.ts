import { z } from 'zod'

export const appErrorSchema = z.object({
  kind: z.string(),
  message: z.string(),
})

export type AppError = z.infer<typeof appErrorSchema>

export const apiSuccessSchema = <TValue extends z.ZodType>(
  dataSchema: TValue,
) =>
  z.object({
    data: dataSchema,
  })
