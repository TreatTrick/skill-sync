import { readFile } from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { resolveSourceLintTargets } from './resolveSourceLintTargets.mjs'

const SOURCE_DIRS = ['src']
const SOURCE_EXTENSIONS = new Set(['.ts', '.svelte'])
const ALLOWED_PATH_PREFIXES = [path.normalize('src/shared/i18n')]
const HAN_CHARACTER_PATTERN = /[\u4e00-\u9fff]/g

const isIncludedSourceFile = (filePath) => !filePath.endsWith('.d.ts')

const getLocation = (content, index) => {
  const before = content.slice(0, index)
  const line = before.split('\n').length
  const column = before.length - before.lastIndexOf('\n')

  return { line, column }
}

const isAllowedFile = (rootDir, filePath) => {
  const relativePath = path.normalize(path.relative(rootDir, filePath))

  return ALLOWED_PATH_PREFIXES.some(
    (allowedPathPrefix) =>
      relativePath === allowedPathPrefix ||
      relativePath.startsWith(`${allowedPathPrefix}${path.sep}`),
  )
}

const getLinePreview = (content, index) => {
  const lineStart = content.lastIndexOf('\n', index - 1) + 1
  const lineEnd = content.indexOf('\n', index)
  const end = lineEnd === -1 ? content.length : lineEnd

  return content.slice(lineStart, end).trim()
}

const checkFile = async (rootDir, filePath) => {
  if (isAllowedFile(rootDir, filePath)) {
    return []
  }

  const content = await readFile(filePath, 'utf8')
  const issues = []
  const reportedLines = new Set()

  for (const match of content.matchAll(HAN_CHARACTER_PATTERN)) {
    const index = match.index ?? 0
    const location = getLocation(content, index)

    if (reportedLines.has(location.line)) {
      continue
    }

    reportedLines.add(location.line)

    issues.push({
      filePath,
      preview: getLinePreview(content, index),
      ...location,
    })
  }

  return issues
}

const run = async () => {
  const rootDir = process.cwd()
  const files = await resolveSourceLintTargets({
    rootDir,
    sourceDirs: SOURCE_DIRS,
    sourceExtensions: SOURCE_EXTENSIONS,
    includeFile: isIncludedSourceFile,
  })

  const issues = (
    await Promise.all(files.map((filePath) => checkFile(rootDir, filePath)))
  ).flat()

  if (issues.length > 0) {
    console.error('i18n literal lint failed:\n')

    for (const issue of issues) {
      console.error(
        `${path.relative(rootDir, issue.filePath)}:${issue.line}:${issue.column} - Move Chinese UI text into src/shared/i18n/locales and reference it with t(...).`,
      )
      console.error(`  ${issue.preview}`)
    }

    process.exitCode = 1
    return
  }

  console.log('i18n literal lint passed.')
}

await run()
