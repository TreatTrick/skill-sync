import { access, readdir } from 'node:fs/promises'
import path from 'node:path'

const pathExists = async (filePath) => {
  try {
    await access(filePath)
    return true
  } catch {
    return false
  }
}

const isInsideDir = (filePath, dirPath) => {
  const relativePath = path.relative(dirPath, filePath)

  return (
    relativePath === '' ||
    (!relativePath.startsWith('..') && !path.isAbsolute(relativePath))
  )
}

const collectSourceFiles = async (dir, sourceExtensions, includeFile) => {
  const entries = await readdir(dir, { withFileTypes: true })
  const files = []

  for (const entry of entries) {
    const entryPath = path.join(dir, entry.name)

    if (entry.isDirectory()) {
      files.push(
        ...(await collectSourceFiles(entryPath, sourceExtensions, includeFile)),
      )
      continue
    }

    if (
      sourceExtensions.has(path.extname(entry.name)) &&
      includeFile(entryPath)
    ) {
      files.push(entryPath)
    }
  }

  return files
}

export const resolveSourceLintTargets = async ({
  rootDir,
  sourceDirs,
  sourceExtensions,
  inputPaths = process.argv.slice(2),
  includeFile = () => true,
}) => {
  const resolvedSourceDirs = sourceDirs.map((sourceDir) =>
    path.join(rootDir, sourceDir),
  )

  if (inputPaths.length === 0) {
    const files = await Promise.all(
      resolvedSourceDirs.map((sourceDir) =>
        collectSourceFiles(sourceDir, sourceExtensions, includeFile),
      ),
    )

    return files.flat()
  }

  const matchedFiles = []

  for (const inputPath of new Set(inputPaths)) {
    const filePath = path.resolve(rootDir, inputPath)

    if (!(await pathExists(filePath))) {
      continue
    }

    if (!sourceExtensions.has(path.extname(filePath))) {
      continue
    }

    if (!includeFile(filePath)) {
      continue
    }

    if (
      !resolvedSourceDirs.some((sourceDir) => isInsideDir(filePath, sourceDir))
    ) {
      continue
    }

    matchedFiles.push(filePath)
  }

  return matchedFiles
}
