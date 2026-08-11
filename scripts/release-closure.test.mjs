import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const ROOT = new URL('../', import.meta.url);

async function source(relativePath) {
    return readFile(new URL(relativePath, ROOT), 'utf8');
}

test('the active release closure delegates to RELEASE-2 rather than historical version scripts', async () => {
    const [packageSource, workflowSource, gateSource] = await Promise.all([
        source('package.json'),
        source('.github/workflows/release.yml'),
        source('tests/release_2_red_gate.test.mjs'),
    ]);
    const version = JSON.parse(packageSource).version;
    assert.match(version, /^\d+\.\d+\.\d+$/);
    assert.match(workflowSource,
        /CRYSTALCANVAS_RELEASE_PUBLISH=1 node --test tests\/release_2_red_gate\.test\.mjs/);
    assert.match(gateSource, /const EVIDENCE_PATH = `release\/v\$\{RELEASE_VERSION\}\/release-evidence\.json`/);
    assert.doesNotMatch(gateSource, /const RELEASE_VERSION = '0\.7\.0'/);
});
