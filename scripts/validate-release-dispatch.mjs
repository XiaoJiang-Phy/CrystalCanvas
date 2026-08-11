// [Overview: Binds a manually dispatched release workflow to one exact source commit and versioned evidence.]
// Implementation of release dispatch identity validation
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { execFileSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';

const [ref, version] = process.argv.slice(2);

if (!/^[0-9a-f]{40}$/i.test(ref ?? '')) {
    throw new Error('release dispatch ref must be a full 40-character commit SHA');
}
if (!/^\d+\.\d+\.\d+$/.test(version ?? '')) {
    throw new Error('release dispatch version must be semantic versioning without a v prefix');
}

const head = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
if (head !== ref) {
    throw new Error(`checked-out HEAD ${head} does not match dispatch ref ${ref}`);
}

const packageJson = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
const evidence = JSON.parse(await readFile(
    new URL(`../release/v${version}/release-evidence.json`, import.meta.url),
    'utf8',
));
if (packageJson.version !== version) {
    throw new Error(`package version ${packageJson.version} does not match dispatch version ${version}`);
}
if (evidence.release_version !== version) {
    throw new Error(`evidence version ${evidence.release_version} does not match dispatch version ${version}`);
}
