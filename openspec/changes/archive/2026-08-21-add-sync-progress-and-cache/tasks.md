## 1. Cache Foundation

- [x] 1.1 Add a Rust skill-pack cache module with versioned index/entry types, local config-directory layout, and atomic index/zip writes.
- [x] 1.2 Implement included-file metadata enumeration using the existing ignore, symlink/reparse, path, file-count, and size safety rules.
- [x] 1.3 Implement cache fingerprints for source identity, metadata, ignore rules, resource limits, and packer/cache algorithm versions.
- [x] 1.4 Implement cache lookup, archive validation, cache-hit/miss accounting, entry replacement, and fallback to full packing on any cache failure.
- [x] 1.5 Implement 1 GiB default capacity accounting, deterministic LRU eviction, oversized-entry handling, and cache statistics/clear operations.

## 2. Sync Engine Integration

- [x] 2.1 Refactor plan preparation and pack batching to use cache hits while preserving `LocalSkillInfo`, warnings, canonical zip bytes, and existing safety limits.
- [x] 2.2 Ensure both preview planning and apply-time re-planning reuse the same cache entries, while remote blob validation still hashes actual upload bytes.
- [x] 2.3 Add a progress reporter boundary that can be used by plan preparation, local action loops, state saving, and cleanup without changing the single-commit transaction flow.
- [x] 2.4 Emit progress events for scan, pack, download, local replace/delete, remote commit, state save, cleanup, completion, failure, and recovery terminal states.
- [x] 2.5 Add cache statistics and clear Tauri commands guarded by the existing sync operation gate.

## 3. Frontend Progress Experience

- [x] 3.1 Add shared schemas/types and Tauri event subscription helpers for operation-scoped progress payloads.
- [x] 3.2 Subscribe before invoking plan/apply commands, render determinate phase progress with current skill and counts, and render remote commit as indeterminate.
- [x] 3.3 Keep failed/recovery terminal progress visible, refresh queries after completion, and omit cancellation controls.
- [x] 3.4 Add translated progress phase, cache hit/miss, cache statistics, clear-cache confirmation, success, and error copy in both locales.

## 4. Cache Settings UI

- [x] 4.1 Add a settings-page cache section showing entry count, occupied bytes, and capacity.
- [x] 4.2 Add a confirmation flow for clearing cache and disable/reject it while a sync operation is active.
- [x] 4.3 Surface clear failures without changing sync state or remote configuration.

## 5. Verification

- [x] 5.1 Add Rust tests for metadata fingerprints, cache hits/misses, invalidation triggers, corrupt entries, atomic recovery, LRU eviction, oversized packs, and clear/stat operations.
- [x] 5.2 Add Rust tests asserting progress payload shape and preserve existing apply tests for single remote commit behavior; runtime event ordering remains covered by the reporter integration path.
- [x] 5.3 Add frontend schema/typecheck and focused component checks for event filtering, determinate/indeterminate rendering, terminal error handling, and cache settings interactions.
- [x] 5.4 Run `npm run typecheck`, `npm run format:check`, `npm run lint`, `npm run build`, `npm run rust:fmt:check`, `npm run rust:clippy`, and `npm run rust:test`.
