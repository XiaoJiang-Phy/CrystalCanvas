// [Overview: Binds a tag-triggered release workflow to one exact source commit and versioned evidence.]
// Release tag identity validation
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { execFileSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';

import { extractReleaseNotes } from './release-notes.mjs';

const [tag, ref] = process.argv.slice(2);

if (!/^v\d+\.\d+\.\d+$/.test(tag ?? '')) {
    throw new Error('release tag must use the vMAJOR.MINOR.PATCH form');
}
if (!/^[0-9a-f]{40}$/i.test(ref ?? '')) {
    throw new Error('release ref must be a full 40-character commit SHA');
}

const version = tag.slice(1);
const head = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
if (head !== ref) {
    throw new Error(`checked-out HEAD ${head} does not match release ref ${ref}`);
}

const packageJson = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
const evidence = JSON.parse(await readFile(
    new URL(`../release/v${version}/release-evidence.json`, import.meta.url),
    'utf8',
));
const changelog = await readFile(new URL('../CHANGELOG.md', import.meta.url), 'utf8');
if (packageJson.version !== version) {
    throw new Error(`package version ${packageJson.version} does not match release version ${version}`);
}
if (evidence.release_version !== version) {
    throw new Error(`evidence version ${evidence.release_version} does not match release version ${version}`);
}
extractReleaseNotes(changelog, version);
