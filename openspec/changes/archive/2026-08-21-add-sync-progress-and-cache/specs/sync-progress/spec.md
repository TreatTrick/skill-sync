## ADDED Requirements

### Requirement: Sync operations SHALL publish stage progress

The sync plan and apply operations SHALL publish `sync-progress` events carrying an operation identifier, operation kind, phase, current count, optional total count, current skill identifier, and whether the progress is determinate. Events SHALL be observable while the original Tauri command is still running.

#### Scenario: Plan preparation reports determinate work

- **WHEN** a sync plan scans and prepares local skills
- **THEN** events identify scan and pack phases with increasing current counts and a total count

#### Scenario: Apply reports local actions

- **WHEN** an apply operation downloads, replaces, or deletes local skills
- **THEN** events identify the active skill and report completed actions against the action total

#### Scenario: Frontend subscribes before invoking

- **WHEN** the sync page starts a plan or apply operation
- **THEN** it registers the progress listener before invoking the Tauri command and filters events by operation identifier

### Requirement: Remote commit progress SHALL be explicitly indeterminate

The sync engine SHALL keep uploads and remote deletions within one `commit_changes` call and SHALL publish an indeterminate `remote_commit` phase while that call is pending. It MUST NOT expose a fabricated percentage for work inside the remote commit.

#### Scenario: Single remote commit

- **WHEN** selected actions require remote changes
- **THEN** the operation publishes one indeterminate remote commit phase and creates at most one remote commit

#### Scenario: Local-only apply

- **WHEN** an apply has only downloads, local deletions, or local state updates
- **THEN** it skips the remote commit phase and reports the local phases normally

### Requirement: Progress terminal states SHALL match command outcomes

The backend SHALL publish a completed terminal event only after state persistence and cleanup succeed. On an error or recovery-required result it SHALL publish a failed or recovery terminal state with the active phase and MUST NOT report 100% successful completion. The first release SHALL NOT expose a cancellation control.

#### Scenario: Successful apply

- **WHEN** the command returns an applied result
- **THEN** the final event identifies completion and the frontend refreshes sync state and plan data

#### Scenario: Recovery required

- **WHEN** a local replace, remote outcome, or state save enters recovery
- **THEN** the final event identifies recovery and the frontend keeps the recovery UI visible
