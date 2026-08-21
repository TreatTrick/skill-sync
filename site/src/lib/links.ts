export const REPO_URL = 'https://github.com/TreatTrick/skill-sync'
export const RELEASES_URL = 'https://github.com/TreatTrick/skill-sync/releases'
export const LATEST_RELEASE_URL =
  'https://github.com/TreatTrick/skill-sync/releases/latest'

export type DownloadItem = {
  platformKey: string
  typeKey: string
  url: string
}

export const DOWNLOADS: DownloadItem[] = [
  {
    platformKey: 'download.winExe',
    typeKey: 'download.typeExe',
    url: 'https://github.com/TreatTrick/skill-sync/releases/download/v1.0.1/Skill.Sync_1.0.1_x64-setup.exe',
  },
  {
    platformKey: 'download.winMsi',
    typeKey: 'download.typeMsi',
    url: 'https://github.com/TreatTrick/skill-sync/releases/download/v1.0.1/Skill.Sync_1.0.1_x64_en-US.msi',
  },
  {
    platformKey: 'download.mac',
    typeKey: 'download.typeDmg',
    url: 'https://github.com/TreatTrick/skill-sync/releases/download/v1.0.1/Skill.Sync_1.0.1_aarch64.dmg',
  },
]
