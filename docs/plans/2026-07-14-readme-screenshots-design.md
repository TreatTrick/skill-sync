# README Screenshots And v0.1.3 Release Design

## Goal

Make the repository landing page more attractive by showing the actual application interface prominently in both README languages, then publish the documentation update as v0.1.3.

## README Layout

- Store the two screenshots in `docs/images/` with their existing descriptive names.
- Add a screenshot section immediately after the badges and before the main introduction so visitors see the product early.
- Show both screenshots vertically at full available width to keep interface text readable.
- Put the English screenshot first in `README.md` and the Chinese screenshot first in `README.zh-CN.md`.
- Use localized headings, captions, and image alternative text in each README.

## Version And Release

- Update the application version from `0.1.2` to `0.1.3` in the five version files already used by previous releases.
- Commit the release changes on `main` with a Conventional Commit message.
- Push `main`, create the annotated `v0.1.3` tag following the repository's existing tag style, and push the tag to `origin`.

## Verification

Per the requested release workflow, do not run lint, build, compilation, or test commands. Verify only the changed files, version values, Git commit, branch push, and remote tag.
