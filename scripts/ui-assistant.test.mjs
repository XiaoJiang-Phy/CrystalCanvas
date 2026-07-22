import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const assistant_paths = [
    'src/App.tsx',
    'src/components/layout/TopNavBar.tsx',
    'src/components/layout/LlmAssistant.tsx',
    'src/components/layout/RightSidebar.tsx',
    'src/components/layout/Shell.tsx',
    'src/hooks/useCameraInteraction.ts',
    'src/utils/tauri-mock.ts',
    'src/ipc/contracts.ts',
    'src/ipc/commands.generated.ts',
];

const sources = Object.fromEntries(await Promise.all(assistant_paths.map(async (path) => [
    path,
    await readFile(new URL('../' + path, import.meta.url), 'utf8'),
])));
const inventory = JSON.parse(await readFile(new URL('../ipc/inventory.json', import.meta.url), 'utf8'));

const app = sources['src/App.tsx'];
const top_nav = sources['src/components/layout/TopNavBar.tsx'];
const assistant = sources['src/components/layout/LlmAssistant.tsx'];
const tauri_mock = sources['src/utils/tauri-mock.ts'];
const contracts = sources['src/ipc/contracts.ts'];
const generated_commands = sources['src/ipc/commands.generated.ts'];

function assistant_open_state_name(source) {
    const assistant_mount = source.match(/<LlmAssistant\b[\s\S]*?\bisOpen=\{([A-Za-z_$][\w$]*)\}[\s\S]*?\/>/);
    assert.ok(assistant_mount, 'App must expose one LlmAssistant mount with an explicit open-state prop');
    return assistant_mount[1];
}

function assistant_mount_block(source) {
    const start = source.indexOf('<LlmAssistant');
    assert.ok(start >= 0, 'App must retain the legacy Assistant access point');
    return source.slice(Math.max(0, start - 240), start + 320);
}

function assistant_mount_tag(source) {
    const mount = source.match(/<LlmAssistant\b[\s\S]*?\/>/);
    assert.ok(mount, 'App must retain one legacy Assistant mount');
    return mount[0];
}

function invoked_commands(source) {
    return [...source.matchAll(/safeInvoke\('([^']+)'/g)].map((match) => match[1]);
}

const open_state = assistant_open_state_name(app);

test('UI-2F starts closed and does not mount Assistant content before explicit access', () => {
    assert.match(
        app,
        new RegExp(`const\\s+\\[\\s*${open_state}\\s*,\\s*[A-Za-z_$][\\w$]*\\s*\\]\\s*=\\s*useState(?:<[^>]+>)?\\(\\s*false\\s*\\)`),
        'the state that controls LlmAssistant must start closed',
    );

    const mount = assistant_mount_block(app);
    assert.match(
        mount,
        new RegExp(`\\{\\s*${open_state}\\s*&&\\s*\\(?\\s*<LlmAssistant\\b`),
        'closed Assistant state must remove the Assistant component from the React tree',
    );
    assert.match(mount, new RegExp(`\\bisOpen=\\{${open_state}\\}`));
    assert.match(top_nav, /onClick=\{onToggleAssistant\}/, 'explicit top-nav access must remain available');
    assert.match(top_nav, /aria-pressed=\{showAssistant\}/, 'the explicit access state must remain observable');
});

test('UI-2F idle workbench has no Assistant IPC, network, timer, or listener work', () => {
    const mount = assistant_mount_block(app);

    assert.match(
        mount,
        new RegExp(`\\{\\s*${open_state}\\s*&&\\s*\\(?\\s*<LlmAssistant\\b`),
        'the idle workbench must not instantiate Assistant effects merely to hide its panel',
    );
    assert.doesNotMatch(app, /\b(?:check_api_key_status|llm_configure|llm_chat|llm_execute_command)\b/,
        'App itself must not own an Assistant command');
    assert.doesNotMatch(assistant, /\b(?:fetch|XMLHttpRequest|WebSocket|EventSource)\s*\(/,
        'the legacy panel must not introduce a direct network client');
    assert.doesNotMatch(assistant, /\b(?:setTimeout|setInterval|requestAnimationFrame)\s*\(/,
        'the legacy panel must not introduce background timer work');
    assert.doesNotMatch(assistant, /\bsafeListen\s*\(/,
        'the legacy panel must not own a Tauri event listener');
});

test('UI-2F closes and unmounts Assistant-owned UI resources', () => {
    const resize_handler = assistant.slice(
        assistant.indexOf('const onResizeMouseDown'),
        assistant.indexOf('if (!isOpen) return null;'),
    );

    assert.match(resize_handler, /window\.addEventListener\('mousemove',\s*onMouseMove\)/,
        'resize owns a window mousemove listener');
    assert.match(resize_handler, /window\.addEventListener\('mouseup',\s*onMouseUp\)/,
        'resize owns a window mouseup listener');
    assert.match(
        assistant,
        /useEffect\(\(\)\s*=>\s*\{[\s\S]*?return\s*\(\)\s*=>\s*\{[\s\S]*?resizingRef\.current\s*=\s*null;[\s\S]*?window\.removeEventListener\('mousemove',[\s\S]*?window\.removeEventListener\('mouseup',[\s\S]*?\}[\s\S]*?\}\s*,\s*\[\s*\]\s*\)/,
        'unmount must cancel an active resize and remove both Assistant-owned global listeners',
    );
    assert.match(
        assistant,
        /useRef(?:<\s*boolean\s*>)?\(\s*false\s*\)[\s\S]*?useEffect\(\(\)\s*=>\s*\{[\s\S]*?\.current\s*=\s*false;[\s\S]*?return\s*\(\)\s*=>\s*\{[\s\S]*?\.current\s*=\s*true;/,
        'pending Assistant UI work needs an unmount disposition guard before it can update local state',
    );
});

test('UI-2F retains explicit preview then manual execution, with no autonomous route', () => {
    const chat_handler = assistant.slice(
        assistant.indexOf('const handleSend'),
        assistant.indexOf('const handleNewChat'),
    );
    const execute_handler = assistant.slice(
        assistant.indexOf('const handleExecute'),
        assistant.indexOf('useEffect(() => {\n        messagesEndRef'),
    );

    assert.match(chat_handler, /JSON\.parse\(response\)/);
    assert.match(chat_handler, /response\.match\(\/```(?:json)?/,
        'a structured action must be parsed from the existing response formats');
    assert.match(chat_handler, /commandJson\s*=\s*JSON\.stringify\(parsedObj, null, 2\)/,
        'an action must become visible preview text before execution');
    assert.match(assistant, /\{msg\.commandJson && \([\s\S]*?<pre[\s\S]*?\{msg\.commandJson\}[\s\S]*?<button[\s\S]*?onClick=\{\(\) => handleExecute\(msg\.commandJson!\)\}/,
        'only a visible prepared command may expose the manual Execute action');
    assert.match(execute_handler, /await safeInvoke\('llm_execute_command', \{ commandJson: json \}\)/);
    assert.doesNotMatch(assistant, /\b(?:auto(?:nomous|execute)|agent(?:ic)?|tool[_-]?call)\b/i,
        'the frozen Assistant must not gain an autonomous execution path');
});

test('UI-2F preserves the fixed LLM command schemas and browser mutation failure', () => {
    const expected_commands = [
        'check_api_key_status',
        'llm_chat',
        'llm_configure',
        'llm_execute_command',
    ];
    const inventory_llm_commands = inventory.backend_commands.filter((command) =>
        command === 'check_api_key_status' || command.startsWith('llm_'),
    ).sort();

    assert.deepEqual(inventory_llm_commands, expected_commands);
    assert.deepEqual(invoked_commands(assistant).sort(), expected_commands);
    assert.deepEqual(inventory.backend_command_args.check_api_key_status, ['providerType']);
    assert.deepEqual(inventory.backend_command_args.llm_chat, ['selectedIndices', 'userMessage']);
    assert.deepEqual(inventory.backend_command_args.llm_configure, ['apiKey', 'model', 'providerType']);
    assert.deepEqual(inventory.backend_command_args.llm_execute_command, ['commandJson']);
    assert.deepEqual(inventory.frontend_commands.llm_chat, ['src/components/layout/LlmAssistant.tsx']);
    assert.deepEqual(inventory.frontend_commands.llm_configure, ['src/components/layout/LlmAssistant.tsx']);
    assert.deepEqual(inventory.frontend_commands.llm_execute_command, ['src/components/layout/LlmAssistant.tsx']);
    assert.match(assistant, /safeInvoke\('check_api_key_status', \{ providerType: newProvider \}\)/);
    assert.match(assistant, /safeInvoke\('llm_configure', \{ providerType, apiKey, model \}\)/);
    assert.match(assistant, /safeInvoke\('llm_chat', \{ userMessage: userMsg\.text, selectedIndices: null \}\)/);
    assert.match(assistant, /safeInvoke\('llm_execute_command', \{ commandJson: json \}\)/);
    assert.match(contracts, /not_in_tauri/);
    assert.match(generated_commands, /llm_chat:\s*'external_io'/);
    assert.match(generated_commands, /llm_configure:\s*'external_io'/);
    assert.match(generated_commands, /llm_execute_command:\s*'committed_mutation'/);
    assert.doesNotMatch(tauri_mock, /IPC_COMMAND_CLASSIFICATION\[[^\]]+\]\s*===\s*'read'\)\s*return\s+undefined;/,
        'browser read commands must not silently resolve undefined; IPC-3B supplies validator-approved fixtures');
    assert.match(tauri_mock, /throw new IpcException\(\{[\s\S]*?code:\s*'not_in_tauri',[\s\S]*?recoverable:\s*false,/,
        'browser LLM mutations must reject with a typed not_in_tauri error instead of resolving as success');
});

test('UI-2F keeps scientific, snapshot, listener, and pointer ownership independent', () => {
    const mount = assistant_mount_block(app);
    const assistant_mount = assistant_mount_tag(app);
    const right_sidebar_mount = app.match(/<RightSidebar\b[\s\S]*?\/>/)?.[0];
    const shell_mount = app.match(/<Shell\b[^>]*>/)?.[0];

    assert.match(app, /const \[crystalState, setCrystalState\] = useState<CrystalState \| null>\(null\)/,
        'App remains the complete snapshot owner');
    assert.equal((app.match(/safeInvoke\('get_crystal_state'/g) || []).length, 1,
        'Assistant work must not introduce a second complete snapshot fetch');
    assert.ok(right_sidebar_mount, 'App must retain the scientific right-sidebar mount');
    assert.match(right_sidebar_mount, /crystalState=\{crystalState\}/);
    assert.doesNotMatch(right_sidebar_mount, /\b(?:showAssistant|isOpen)=\{/,
        'Assistant state must not become right-scientific-tools state');
    assert.ok(shell_mount, 'App must retain the workbench shell');
    assert.doesNotMatch(shell_mount, /\b(?:showAssistant|isOpen)=\{/,
        'Assistant state must not become WGPU viewport state');
    assert.doesNotMatch(assistant_mount, /\bviewportRef\b|\bcrystalState\b|\bsafeListen\b/,
        'Assistant receives neither pointer, snapshot, nor Tauri-listener ownership');
    assert.doesNotMatch(assistant, /\bsafeInvoke\('get_crystal_state'/,
        'Assistant cannot acquire complete snapshot ownership');
    assert.doesNotMatch(assistant, /\bonPointer(?:Down|Move|Up|Cancel)\s*=/,
        'Assistant cannot become a 3D pointer-event target');
    assert.match(sources['src/hooks/useCameraInteraction.ts'], /const el = viewportRef\.current;/);
    assert.match(sources['src/components/layout/Shell.tsx'], /ref=\{viewportRef\}/);
    assert.doesNotMatch(sources['src/components/layout/RightSidebar.tsx'], /LlmAssistant/,
        'scientific tool ownership stays outside the legacy Assistant');
});

test('UI-2F rejects persistent Assistant expansion surfaces', () => {
    assert.doesNotMatch(assistant, /\b(?:localStorage|sessionStorage|indexedDB|BroadcastChannel)\b/,
        'the frozen Assistant must not add persistent conversation or memory storage');
    assert.doesNotMatch(assistant, /\b(?:rag|retrieval|embedding|vectorStore|plugin)\b/i,
        'the frozen Assistant must not add RAG, retrieval, or plugin surfaces');
    assert.doesNotMatch(assistant, /from\s+['"][^'"]*(?:memory|rag|plugin|tool)[^'"]*['"]/i,
        'the frozen Assistant must not import a new product-expansion module');
});
