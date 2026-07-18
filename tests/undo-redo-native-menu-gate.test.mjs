import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const [main_source, menu_hook_source] = await Promise.all([
    readFile(new URL('../src-tauri/src/main.rs', import.meta.url), 'utf8'),
    readFile(new URL('../src/hooks/useTauriMenu.ts', import.meta.url), 'utf8'),
]);

function menu_action_branch(action) {
    const start = menu_hook_source.indexOf(`action === '${action}'`);
    assert.notEqual(start, -1, `missing ${action} menu action`);
    const end = menu_hook_source.indexOf('} else if', start);
    return menu_hook_source.slice(start, end === -1 ? undefined : end);
}

test('native undo and redo use application-owned menu IDs and accelerators', () => {
    assert.doesNotMatch(main_source, /PredefinedMenuItem::undo\(/);
    assert.doesNotMatch(main_source, /PredefinedMenuItem::redo\(/);
    assert.match(main_source, /MenuItem::with_id\(\s*app,\s*"menu_undo",\s*"Undo",\s*true,\s*Some\("CommandOrControl\+Z"\)/);
    assert.match(main_source, /MenuItem::with_id\(\s*app,\s*"menu_redo",\s*"Redo",\s*true,\s*Some\("CommandOrControl\+Shift\+Z"\)/);
});

test('native undo and redo menu events are forwarded exactly to their frontend actions', () => {
    for (const [id, action] of [['menu_undo', 'undo'], ['menu_redo', 'redo']]) {
        assert.match(
            main_source,
            new RegExp(`"${id}"\\s*=>\\s*\\{[\\s\\S]{0,240}?emit\\("menu-action", "${action}"\\)`),
        );
    }
});

test('the menu listener is the sole desktop undo and redo dispatch path', () => {
    for (const action of ['undo', 'redo']) {
        assert.match(menu_action_branch(action), new RegExp(`safeInvoke\\('${action}'\\)\\.catch\\(console\\.error\\)`));
        assert.equal((menu_hook_source.match(new RegExp(`safeInvoke\\('${action}'`, 'g')) ?? []).length, 1);
    }
    assert.doesNotMatch(menu_hook_source, /window\.addEventListener\('keydown'/);
    assert.doesNotMatch(menu_hook_source, /window\.removeEventListener\('keydown'/);
});
