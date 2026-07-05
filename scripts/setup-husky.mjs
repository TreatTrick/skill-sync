import { access } from 'node:fs/promises'
import { spawn } from 'node:child_process'
import process from 'node:process'

const hasGitDirectory = async () => {
  try {
    await access('.git')
    return true
  } catch {
    return false
  }
}

const runHuskyInstall = async () =>
  new Promise((resolve, reject) => {
    const child = spawn('npx', ['husky', 'install'], {
      stdio: 'inherit',
      shell: process.platform === 'win32',
    })

    child.on('close', (code) => {
      if (code === 0) {
        resolve()
        return
      }

      reject(new Error(`husky install failed with exit code ${code}`))
    })
  })

if (await hasGitDirectory()) {
  await runHuskyInstall()
} else {
  console.log('Skipping husky install because .git was not found.')
}
