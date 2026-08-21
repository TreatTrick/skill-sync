# Purpose

Define the local skill-pack cache capacity, observability, and clearing behavior.

## Requirements

### Requirement: Cache size SHALL be bounded with LRU eviction

The local skill-pack cache SHALL enforce a default total capacity of 1 GiB. After adding an entry, the cache SHALL evict least-recently-used entries until the total recorded size is within the limit. An individual pack larger than the capacity SHALL remain usable for the current operation but SHALL NOT be retained.

#### Scenario: Capacity exceeded

- **WHEN** writing a new cache entry would exceed the configured capacity
- **THEN** the oldest entries are removed until the capacity is respected

#### Scenario: Oversized individual pack

- **WHEN** a canonical zip is larger than the cache capacity
- **THEN** the sync uses it for the current operation and does not retain it in the cache

### Requirement: Users SHALL be able to inspect and clear the cache

The application SHALL provide a settings view that displays cache entry count, current bytes, and capacity, and provides a clear-cache action. Clearing SHALL remove only reconstructible cache artifacts and SHALL preserve config, sync state, recovery journals, and remote content.

#### Scenario: View cache statistics

- **WHEN** the settings page requests cache information
- **THEN** it displays the current entry count, occupied bytes, and configured capacity

#### Scenario: Clear cache

- **WHEN** the user confirms the clear-cache action while no sync operation is running
- **THEN** all cache entries are removed and sync state and remote data remain unchanged

#### Scenario: Clear blocked during sync

- **WHEN** a sync operation is active
- **THEN** the clear-cache action is disabled or rejected until the operation finishes
