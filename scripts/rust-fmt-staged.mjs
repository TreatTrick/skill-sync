import { spawnSync } from 'node:child_process'
import process from 'node:process'

// pre-commit 只检查 staged 的 Rust 文件，避免对整个 crate 跑 fmt。
const RUST_PREFIX = 'src-tauri/'

const run = () => {
  // 列出 staged 且未被删除的文件
  const diff = spawnSync(
    'git',
    ['diff', '--cached', '--name-only', '--diff-filter=d'],
    { encoding: 'utf8' },
  )

  if (diff.error) {
    console.error('Failed to read staged files:', diff.error)
    process.exitCode = 1
    return
  }

  const stagedRustFiles = diff.stdout
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.endsWith('.rs') && line.startsWith(RUST_PREFIX))

  if (stagedRustFiles.length === 0) {
    console.log('rust:fmt:staged skipped (no staged .rs files).')
    return
  }

  console.log(
    `Checking ${stagedRustFiles.length} staged Rust file(s) with cargo fmt...`,
  )

  // 透传退出码：cargo fmt --check 在发现未格式化文件时返回非零
  const fmt = spawnSync(
    'cargo',
    [
      'fmt',
      '--manifest-path',
      'src-tauri/Cargo.toml',
      '--',
      '--check',
      ...stagedRustFiles,
    ],
    { stdio: 'inherit' },
  )

  if (fmt.error) {
    console.error('Failed to run cargo fmt:', fmt.error)
    process.exitCode = 1
    return
  }

  process.exitCode = fmt.status ?? 1
}

run()
