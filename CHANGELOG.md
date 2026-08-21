# Changelog

All notable changes to Skill Sync are documented here.

## [1.0.1] - 2026-08-21

### Added

- Added skill-pack caching with fingerprint validation, cache statistics, deterministic LRU eviction, and a clear-cache action.
- Added operation-scoped progress reporting across scanning, packing, downloads, local changes, remote commits, recovery, and completion states.
- Added progress feedback and cache management controls to the Sync and Settings pages in English and Simplified Chinese.
- Added a bilingual GitHub Pages homepage with SEO metadata, sitemap, and direct installer links.

### Changed

- Preview planning and apply-time re-planning now reuse compatible skill-pack cache entries while validating the bytes uploaded to GitHub.
- Updated application, website, Rust crate, documentation, and installer links to version `1.0.1`.

## [1.0.0] - 2026-07-11

- Initial public release.
