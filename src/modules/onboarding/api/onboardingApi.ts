import { invokeCmd } from '@/shared/lib'

import {
  gitCheckSchema,
  remoteCheckSchema,
  type GitCheck,
  type RemoteCheck,
} from '../schemas/onboarding'

export const checkGit = async (): Promise<GitCheck> =>
  gitCheckSchema.parse(await invokeCmd<unknown>('check_git'))

export const checkRemote = async (remote: string): Promise<RemoteCheck> =>
  remoteCheckSchema.parse(await invokeCmd<unknown>('check_remote', { remote }))

export const prepareRepo = async (
  localPath: string,
  remote: string,
  branch: string,
): Promise<void> => {
  await invokeCmd('prepare_repo', { localPath, remote, branch })
}
