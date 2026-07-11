import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { assert_contract, create_inventory } from './ipc-inventory.mjs';

const root = resolve(fileURLToPath(new URL('..', import.meta.url)));
const expected = `${JSON.stringify(await create_inventory(), null, 2)}\n`;
const inventory_path = resolve(root, 'ipc/inventory.json');
const actual = await readFile(inventory_path, 'utf8');

assert_contract(JSON.parse(actual));
if (actual !== expected) {
    throw new Error('ipc/inventory.json is stale; run npm run ipc:inventory and commit the result');
}
assert_contract(JSON.parse(expected));
process.stdout.write('IPC contract inventory is consistent.\n');
