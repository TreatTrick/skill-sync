import { readdir, readFile } from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'

const RESPONSIVE_VARIANTS = new Set(['sm', 'md', 'lg', 'xl', '2xl'])
const SOURCE_DIRS = ['src']
const SOURCE_EXTENSIONS = new Set(['.css', '.ts', '.svelte'])
const STATIC_GRID_COLUMN_MIN = 2
const LARGE_ARBITRARY_GRID_PX = 240
const LARGE_WIDTH_PX = 768
const LARGE_MIN_WIDTH_PX = 1024
const LARGE_HEIGHT_PX = 480
const LARGE_MIN_HEIGHT_PX = 480
const LARGE_MAX_WIDTH_PX = 1536
const LARGE_MAX_HEIGHT_PX = 640
const LARGE_CSS_WIDTH_PX = 1024

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

const getMaxPxValue = (value) => {
  const matches = value.matchAll(/(\d+)px/g)
  let maxValue = 0

  for (const match of matches) {
    maxValue = Math.max(maxValue, Number(match[1]))
  }

  return maxValue
}

const getArbitraryValue = (baseClass) => {
  const match = baseClass.match(/\[(.+)\]$/)

  return match?.[1] ?? ''
}

const getLengthInPx = (value) => {
  const pxMatch = value.match(/^(\d+(?:\.\d+)?)px$/)

  if (pxMatch) {
    return Number(pxMatch[1])
  }

  const remMatch = value.match(/^(\d+(?:\.\d+)?)rem$/)

  if (remMatch) {
    return Number(remMatch[1]) * 16
  }

  return null
}

const getTokenParts = (token) => {
  const parts = token.split(':')
  const baseClass = parts.at(-1) ?? token
  const variants = parts.slice(0, -1)

  return { baseClass, variants }
}

const hasResponsiveVariant = (variants) =>
  variants.some((variant) => RESPONSIVE_VARIANTS.has(variant))

const addIssue = (issues, filePath, content, index, message) => {
  const location = getLocation(content, index)

  issues.push({
    filePath,
    message,
    ...location,
  })
}

const checkCssWidthRules = (filePath, content, issues) => {
  const widthRulePattern = /(^|\n)\s*(min-width|width)\s*:\s*(\d+)px\s*;/g

  for (const match of content.matchAll(widthRulePattern)) {
    const property = match[2]
    const width = Number(match[3])
    const threshold =
      property === 'min-width' ? LARGE_MIN_WIDTH_PX : LARGE_CSS_WIDTH_PX

    if (width >= threshold) {
      addIssue(
        issues,
        filePath,
        content,
        match.index ?? 0,
        `Avoid global fixed ${property}: ${width}px; move minimum widths to local overflow containers or responsive classes.`,
      )
    }
  }
}

const checkTailwindTokens = (filePath, content, issues) => {
  const tokenPattern =
    /(?:[a-z0-9-]+:)*(?:grid-cols-\[[^\]\s'"`]+\]|grid-cols-\d+|(?:w|h|min-w|min-h|max-w|max-h)-\[[^\]\s'"`]+\]|text-\[[^\]\s'"`]+\]|rounded-\[[^\]\s'"`]+\])/g

  for (const match of content.matchAll(tokenPattern)) {
    const token = match[0]
    const { baseClass, variants } = getTokenParts(token)
    const responsive = hasResponsiveVariant(variants)
    const index = match.index ?? 0

    if (baseClass.startsWith('grid-cols-[')) {
      const maxPx = getMaxPxValue(baseClass)

      if (!responsive && maxPx >= LARGE_ARBITRARY_GRID_PX) {
        addIssue(
          issues,
          filePath,
          content,
          index,
          `Fixed grid template "${token}" must be behind a responsive variant and paired with a small-screen fallback such as grid-cols-1.`,
        )
      }

      continue
    }

    const gridColumnMatch = baseClass.match(/^grid-cols-(\d+)$/)

    if (gridColumnMatch) {
      const columns = Number(gridColumnMatch[1])

      if (!responsive && columns >= STATIC_GRID_COLUMN_MIN) {
        addIssue(
          issues,
          filePath,
          content,
          index,
          `Static "${token}" should use a small-screen fallback and responsive column variants, for example grid-cols-1 sm:grid-cols-2 xl:${token}.`,
        )
      }

      continue
    }

    const dimensionMatch = baseClass.match(
      /^(w|h|min-w|min-h|max-w|max-h)-\[(.+)\]$/,
    )

    if (dimensionMatch && !responsive) {
      const property = dimensionMatch[1]
      const lengthValue = dimensionMatch[2]
      const lengthInPx = getLengthInPx(lengthValue)
      const thresholdByProperty = {
        h: LARGE_HEIGHT_PX,
        'max-h': LARGE_MAX_HEIGHT_PX,
        'max-w': LARGE_MAX_WIDTH_PX,
        'min-h': LARGE_MIN_HEIGHT_PX,
        'min-w': LARGE_MIN_WIDTH_PX,
        w: LARGE_WIDTH_PX,
      }
      const threshold = thresholdByProperty[property]

      if (lengthInPx === null || lengthInPx >= threshold) {
        addIssue(
          issues,
          filePath,
          content,
          index,
          `Large arbitrary size "${token}" should use a standard Tailwind size token, local overflow fallback, or responsive variants.`,
        )
      }

      continue
    }

    if (baseClass.startsWith('text-[')) {
      addIssue(
        issues,
        filePath,
        content,
        index,
        `Arbitrary text size "${token}" is not allowed; use stable Tailwind text sizes such as text-sm, text-base, text-xl, or text-2xl.`,
      )

      continue
    }

    if (baseClass.startsWith('rounded-[')) {
      const arbitraryValue = getArbitraryValue(baseClass)

      addIssue(
        issues,
        filePath,
        content,
        index,
        `Arbitrary radius "${token}" is not allowed; use standard radius tokens such as rounded-md, rounded-lg, rounded-full, or define a semantic token. Found ${arbitraryValue}.`,
      )
    }
  }
}

const checkTables = (filePath, content, issues) => {
  const tablePattern = /<table\b[\s\S]*?>/g

  for (const match of content.matchAll(tablePattern)) {
    const tableTag = match[0]
    const index = match.index ?? 0
    const nearbyBefore = content.slice(Math.max(0, index - 500), index)

    if (!tableTag.includes('min-w-[')) {
      addIssue(
        issues,
        filePath,
        content,
        index,
        'Table elements must define a local min-w-[...] so narrow screens can scroll instead of crushing columns.',
      )
    }

    if (!nearbyBefore.includes('overflow-x-auto')) {
      addIssue(
        issues,
        filePath,
        content,
        index,
        'Table elements must be wrapped in an overflow-x-auto container for narrow screens.',
      )
    }
  }
}

const checkFile = async (filePath) => {
  const content = await readFile(filePath, 'utf8')
  const issues = []

  if (filePath.endsWith('.css')) {
    checkCssWidthRules(filePath, content, issues)
  }

  checkTailwindTokens(filePath, content, issues)

  if (filePath.endsWith('.svelte')) {
    checkTables(filePath, content, issues)
  }

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
    await Promise.all(files.map((filePath) => checkFile(filePath)))
  ).flat()

  if (issues.length > 0) {
    console.error('Responsive layout lint failed:\n')

    for (const issue of issues) {
      console.error(
        `${path.relative(rootDir, issue.filePath)}:${issue.line}:${issue.column} - ${issue.message}`,
      )
    }

    process.exitCode = 1
    return
  }

  console.log('Responsive layout lint passed.')
}

await run()
