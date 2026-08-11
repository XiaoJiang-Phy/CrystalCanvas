// [Overview: Extracts one version's public release notes from CHANGELOG.md.]
// Release-note extraction for tag-driven GitHub releases
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { readFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';

const VERSION_PATTERN = /^\d+\.\d+\.\d+$/;
const SECTION_PATTERN = /^## \[(\d+\.\d+\.\d+)\] - \d{4}-\d{2}-\d{2}$/;

export function extractReleaseNotes(changelog, version) {
    if (!VERSION_PATTERN.test(version ?? '')) {
        throw new Error('release-note version must be semantic versioning without a v prefix');
    }

    const lines = changelog.replaceAll('\r\n', '\n').split('\n');
    const matchingHeaders = lines
        .map((line, index) => ({ match: line.match(SECTION_PATTERN), index }))
        .filter(({ match }) => match?.[1] === version);
    if (matchingHeaders.length !== 1) {
        throw new Error(`CHANGELOG.md must contain exactly one release section for ${version}`);
    }

    const start = matchingHeaders[0].index + 1;
    const nextHeader = lines.findIndex((line, index) => index >= start && SECTION_PATTERN.test(line));
    const end = nextHeader === -1 ? lines.length : nextHeader;
    const notes = lines.slice(start, end).join('\n').trim();
    if (!notes) {
        throw new Error(`CHANGELOG.md release section for ${version} must not be empty`);
    }
    return `${notes}\n`;
}

async function main() {
    const [version] = process.argv.slice(2);
    const changelog = await readFile(new URL('../CHANGELOG.md', import.meta.url), 'utf8');
    process.stdout.write(extractReleaseNotes(changelog, version));
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
    await main();
}
