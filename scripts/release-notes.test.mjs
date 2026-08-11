import assert from 'node:assert/strict';
import test from 'node:test';

import { extractReleaseNotes } from './release-notes.mjs';

const CHANGELOG = `# Changelog

## [0.8.0] - 2026-08-11

### Added

- Field scenes.

### Changed

- Tag-driven releases.

## [0.7.0] - 2026-07-28

### Added

- Publication export.
`;

test('release notes contain only the requested changelog body', () => {
    assert.equal(extractReleaseNotes(CHANGELOG, '0.8.0'), `### Added

- Field scenes.

### Changed

- Tag-driven releases.
`);
});

test('release notes reject missing, duplicate, empty, or malformed versions', () => {
    assert.throws(() => extractReleaseNotes(CHANGELOG, 'v0.8.0'), /semantic versioning/);
    assert.throws(() => extractReleaseNotes(CHANGELOG, '0.9.0'), /exactly one/);
    assert.throws(() => extractReleaseNotes(`${CHANGELOG}\n## [0.8.0] - 2026-08-12\n\n- Duplicate.\n`, '0.8.0'),
        /exactly one/);
    assert.throws(() => extractReleaseNotes('## [0.8.0] - 2026-08-11\n\n', '0.8.0'), /must not be empty/);
});
