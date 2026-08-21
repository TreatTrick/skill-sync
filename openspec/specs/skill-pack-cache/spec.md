# Purpose

Define correctness, invalidation, and recovery rules for cached skill packs.

## Requirements

### Requirement: Unchanged skills SHALL reuse validated pack cache entries

The sync engine SHALL persist successful canonical skill packs with their hash, zip size, warnings, source identity, included file metadata, packer/cache versions, ignore-rule fingerprint, and resource-limit fingerprint. A cache hit SHALL require all fingerprints and file metadata to match, the zip to exist with the recorded size, and the archive to be readable.

#### Scenario: Cache hit avoids repacking

- **WHEN** a skill's included paths, file types, sizes, modification timestamps, source identity, rules, limits, and algorithm versions match a valid cache entry
- **THEN** the plan reuses the cached zip, hash, size, and warnings without reading and recompressing the skill contents

#### Scenario: Changed metadata causes a miss

- **WHEN** an included file is added, removed, renamed, changed in size or modification timestamp, or a source identity changes
- **THEN** the engine performs the existing full read, canonical pack, warning scan, and hash calculation and replaces the cache entry

### Requirement: Cache invalidation SHALL protect sync correctness

The cache SHALL be invalidated for packer/cache algorithm changes, ignore-rule changes, resource-limit changes, missing or malformed index data, missing or size-mismatched zip files, unsafe file metadata, or archive-read failures. A cache failure SHALL fall back to full packing and MUST NOT fail an otherwise valid sync solely because the cache is unavailable.

#### Scenario: Ignore rules invalidate an entry

- **WHEN** the configured ignore rules differ from the fingerprint stored with an entry
- **THEN** the entry is not reused and the skill is repacked with the new rules

#### Scenario: Corrupt cache falls back

- **WHEN** a cached zip is missing, has a different size, or cannot be opened as a valid archive
- **THEN** the engine removes or ignores that entry, repacks the skill, and continues the sync

### Requirement: Cache writes SHALL be recoverable and local

Cache artifacts SHALL live only under the local application configuration directory, SHALL be written through temporary files and atomic replacement, and SHALL never be included in the remote manifest or sync state. Actual remote uploads SHALL continue to validate blob bytes against their expected SHA-256.

#### Scenario: Interrupted cache write

- **WHEN** the process stops during an entry or index write
- **THEN** the next operation ignores incomplete artifacts and can rebuild the affected entry without changing sync state

#### Scenario: Upload integrity remains enforced

- **WHEN** a cached pack is selected for upload
- **THEN** the remote store validates the uploaded bytes against the cached expected hash before committing
