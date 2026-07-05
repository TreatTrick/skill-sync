import { readdir, readFile } from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'

const SOURCE_DIRS = ['src']
const SOURCE_EXTENSIONS = new Set(['.css', '.ts', '.tsx'])
const TOKEN_DEFINITION_FILES = new Set([path.normalize('src/index.css')])
const PALETTE_NAMES = [
  'slate',
  'gray',
  'zinc',
  'neutral',
  'stone',
  'red',
  'orange',
  'amber',
  'yellow',
  'lime',
  'green',
  'emerald',
  'teal',
  'cyan',
  'sky',
  'blue',
  'indigo',
  'violet',
  'purple',
  'fuchsia',
  'pink',
  'rose',
  'white',
  'black',
]
const COLOR_UTILITY_PREFIXES = [
  'bg',
  'text',
  'border',
  'ring',
  'outline',
  'decoration',
  'accent',
  'caret',
  'fill',
  'stroke',
  'from',
  'via',
  'to',
  'shadow',
]
const COLOR_FUNCTION_PATTERN = /(?:#[0-9a-fA-F]{3,8}|rgba?\(|hsla?\()/g
const PALETTE_UTILITY_PATTERN = new RegExp(
  String.raw`(?:[a-z0-9-]+:)*(?:${COLOR_UTILITY_PREFIXES.join('|')})-(?:${PALETTE_NAMES.join('|')})(?:-\d{2,3})?(?:\/\d+)?\b`,
  'g',
)

const collectSourceFiles = async (dir) => {
  const entries = await readdir(dir, { withFileTypes: true })
  const files = []

  for (const entry of entries) {
    const entryPath = path.join(dir, entry.name)

    if (entry.isDirectory()) {
      files.push(...(await collectSourceFiles(entryPath)))
      continue
    }

    if (SOURCE_EXTENSIONS.has(path.extname(entry.name))) {
      files.push(entryPath)
    }
  }

  return files
}

const getLocation = (content, index) => {
  const before = content.slice(0, index)
  const line = before.split('\n').length
  const column = before.length - before.lastIndexOf('\n')

  return { line, column }
}

const addIssue = (issues, filePath, content, index, message) => {
  const location = getLocation(content, index)

  issues.push({
    filePath,
    message,
    ...location,
  })
}

const isTokenDefinitionFile = (rootDir, filePath) => {
  const relativePath = path.normalize(path.relative(rootDir, filePath))

  return TOKEN_DEFINITION_FILES.has(relativePath)
}

const checkPaletteUtilities = (filePath, content, issues) => {
  for (const match of content.matchAll(PALETTE_UTILITY_PATTERN)) {
    addIssue(
      issues,
      filePath,
      content,
      match.index ?? 0,
      `Color utility "${match[0]}" uses Tailwind palette colors directly. Use semantic tokens such as bg-surface, text-foreground, border-border, bg-primary, text-warning, or text-destructive.`,
    )
  }
}

const checkLiteralColors = (filePath, content, issues) => {
  for (const match of content.matchAll(COLOR_FUNCTION_PATTERN)) {
    addIssue(
      issues,
      filePath,
      content,
      match.index ?? 0,
      `Literal color "${match[0]}" is only allowed in src/index.css token definitions. Add or reuse a semantic color token instead.`,
    )
  }
}

const checkFile = async (rootDir, filePath) => {
  const content = await readFile(filePath, 'utf8')
  const issues = []

  if (isTokenDefinitionFile(rootDir, filePath)) {
    return issues
  }

  checkPaletteUtilities(filePath, content, issues)
  checkLiteralColors(filePath, content, issues)

  return issues
}

const run = async () => {
  const rootDir = process.cwd()
  const files = (
    await Promise.all(
      SOURCE_DIRS.map((sourceDir) =>
        collectSourceFiles(path.join(rootDir, sourceDir)),
      ),
    )
  ).flat()

  const issues = (
    await Promise.all(files.map((filePath) => checkFile(rootDir, filePath)))
  ).flat()

  if (issues.length > 0) {
    console.error('Color token lint failed:\n')

    for (const issue of issues) {
      console.error(
        `${path.relative(rootDir, issue.filePath)}:${issue.line}:${issue.column} - ${issue.message}`,
      )
    }

    process.exitCode = 1
    return
  }

  console.log('Color token lint passed.')
}

await run()
