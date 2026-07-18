import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const persistent_surface_paths = [
    'src/components/layout/Shell.tsx',
    'src/components/layout/TopNavBar.tsx',
    'src/components/layout/BottomStatusBar.tsx',
    'src/components/layout/LeftSidebar.tsx',
    'src/components/layout/RightSidebar.tsx',
    'src/components/layout/LlmAssistant.tsx',
];

const [index_css, package_json, persistent_surfaces] = await Promise.all([
    readFile(new URL('../src/index.css', import.meta.url), 'utf8'),
    readFile(new URL('../package.json', import.meta.url), 'utf8'),
    Promise.all(persistent_surface_paths.map(async (path) => [
        path,
        await readFile(new URL(`../${path}`, import.meta.url), 'utf8'),
    ])),
]);

const ui_1b_source_paths = [
    'src/App.tsx',
    'src/components/layout/Shell.tsx',
    'src/components/layout/TopNavBar.tsx',
    'src/components/layout/BottomStatusBar.tsx',
    'src/hooks/useCameraInteraction.ts',
];

const ui_1b_sources = Object.fromEntries(await Promise.all(ui_1b_source_paths.map(async (path) => [
    path,
    await readFile(new URL('../' + path, import.meta.url), 'utf8'),
])));

const package_manifest = JSON.parse(package_json);
function strip_comments(source) {
    return source
        .replace(/\/\*[\s\S]*?\*\//g, '')
        .replace(/^\s*\/\/.*$/gm, '');
}

const persistent_sources = Object.fromEntries(persistent_surfaces.map(([path, source]) => [
    path,
    strip_comments(source),
]));

const ui_1b_clean_sources = Object.fromEntries(Object.entries(ui_1b_sources).map(([path, source]) => [
    path,
    strip_comments(source),
]));

const required_tokens = [
    '--cc-canvas',
    '--cc-chrome',
    '--cc-panel',
    '--cc-field',
    '--cc-border',
    '--cc-text',
    '--cc-muted',
    '--cc-accent',
    '--cc-danger',
];

function css_block(source, selector) {
    const escaped_selector = selector.replace('.', '\\.').replace(':', '\\:');
    const match = source.match(new RegExp(escaped_selector + '\\s*\\{([\\s\\S]*?)\\}'));
    assert.ok(match, 'missing ' + selector + ' token block');
    return match[1];
}

function alpha_is_opaque(alpha) {
    return /^(?:1(?:\.0+)?|100(?:\.0+)?%)$/.test(alpha.trim());
}

function is_opaque_css_color(value) {
    const color = value.trim().toLowerCase();
    if (/^#[0-9a-f]{3}(?:[0-9a-f]{3})?$/.test(color)) return true;

    const functional = color.match(/^(rgb|rgba|hsl|hsla)\((.*)\)$/);
    if (!functional) return false;

    const [, function_name, body] = functional;
    const slash_index = body.lastIndexOf('/');
    if (slash_index >= 0) return alpha_is_opaque(body.slice(slash_index + 1));
    if (!function_name.endsWith('a')) return true;

    const legacy_parts = body.split(',');
    return legacy_parts.length === 4 && alpha_is_opaque(legacy_parts[3]);
}

function assert_complete_opaque_token_block(source, selector) {
    const block = css_block(source, selector);
    for (const token of required_tokens) {
        const match = block.match(new RegExp(token + '\\s*:\\s*([^;}]+)\\s*;', 'i'));
        assert.ok(match, selector + ' is missing ' + token);
        assert.equal(is_opaque_css_color(match[1]), true, selector + ' ' + token + ' must be opaque');
    }
}

const forbidden_persistent_effects = [
    ['backdrop blur', /\bbackdrop-blur(?:-[\w\[\]/.-]+)?\b/],
    ['decorative gradient', /\b(?:bg-)?gradient(?:-to-[\w-]+)?\b/],
    ['presentation shadow', /\bshadow-(?:lg|xl|2xl)\b/],
    ['unbounded transition', /\btransition-all\b/],
    ['over-budget transition duration', /\bduration-(?:200|300|500|700|1000)\b/],
];

function jsx_button_blocks(source) {
    return source.match(/<button\b[\s\S]*?<\/button>/g) || [];
}

function assert_no_chrome_pointer_target(source, path) {
    assert.doesNotMatch(source, /\bonPointer(?:Down|Move|Up|Cancel)\s*=/, path + ' binds a JSX pointer handler');
    assert.doesNotMatch(
        source,
        /\baddEventListener\(\s*['"]pointer(?:down|move|up|cancel)['"]/,
        path + ' registers a direct pointer listener',
    );
}

test('UI-1B keeps top and bottom chrome compact and free of presentation effects', () => {
    const top_nav = ui_1b_clean_sources['src/components/layout/TopNavBar.tsx'];
    const bottom_status = ui_1b_clean_sources['src/components/layout/BottomStatusBar.tsx'];

    for (const [path, source] of [
        ['src/components/layout/TopNavBar.tsx', top_nav],
        ['src/components/layout/BottomStatusBar.tsx', bottom_status],
    ]) {
        assert.doesNotMatch(source, /\bbackdrop-blur(?:-[\w\[\]/.-]+)?\b/, path + ' uses blur');
        assert.doesNotMatch(source, /\b(?:bg-)?gradient(?:-to-[\w-]+)?\b/, path + ' uses a gradient');
        assert.doesNotMatch(source, /\banimate-[\w\[\]-]+\b/, path + ' creates a continuous animation');
        assert.doesNotMatch(source, /\p{Extended_Pictographic}/u, path + ' uses emoji');
    }

    assert.match(top_nav, /\bh-12\b/, 'top chrome must stay within the 48px budget');
    assert.match(bottom_status, /\bh-7\b/, 'bottom status must stay within the 28px budget');
});

test('UI-1B reserves collision-free chrome tracks at the 1280px desktop baseline', () => {
    const top_nav = ui_1b_clean_sources['src/components/layout/TopNavBar.tsx'];
    const bottom_status = ui_1b_clean_sources['src/components/layout/BottomStatusBar.tsx'];

    assert.match(
        top_nav,
        /grid-cols-\[minmax\(0,1fr\)_auto_minmax\(0,1fr\)\]/,
        'top chrome needs left, centered view, and right tracks that cannot overlap',
    );
    assert.match(top_nav, /\bmin-w-0\b/, 'top chrome tracks must be shrinkable');
    assert.match(top_nav, /\bjustify-end\b/, 'right application actions must remain in the right track');
    assert.match(
        bottom_status,
        /grid-cols-\[minmax\(0,1fr\)_auto\]/,
        'status chrome needs independent summary and counter tracks',
    );
    assert.match(bottom_status, /\bmin-w-0\b/, 'status summary track must be shrinkable');
});

test('UI-1B provides text semantics for every icon-only top-nav control', () => {
    const top_nav = ui_1b_clean_sources['src/components/layout/TopNavBar.tsx'];
    const icon_only_buttons = jsx_button_blocks(top_nav).filter((button) =>
        /<(?:Bot|Sun|Moon|Settings)\b/.test(button),
    );

    assert.ok(icon_only_buttons.length > 0, 'expected icon-only controls in the top navigation');
    for (const button of icon_only_buttons) {
        assert.match(button, /aria-label=/, 'icon-only control needs an accessible name');
        assert.match(button, /title=/, 'icon-only control needs a tooltip');
    }
    assert.match(top_nav, /aria-label=\{tooltip\}/, 'interaction mode controls must retain their accessible-name outlet');
    assert.match(top_nav, /title=\{tooltip\}/, 'interaction mode controls must retain their tooltip outlet');
    assert.match(top_nav, /tooltip="Select"/);
    assert.match(top_nav, /tooltip="Move"/);
    assert.match(top_nav, /tooltip="Rotate"/);
    assert.match(top_nav, /tooltip="Measure\/Select"/);
});

test('UI-1B exposes the Assistant open state without relying on color', () => {
    const top_nav = ui_1b_clean_sources['src/components/layout/TopNavBar.tsx'];
    const assistant_toggle = jsx_button_blocks(top_nav).find((button) => /<Bot\b/.test(button));

    assert.ok(assistant_toggle, 'expected an Assistant toggle button');
    assert.match(
        assistant_toggle,
        /aria-pressed=\{showAssistant\}/,
        'Assistant open state must be exposed beyond its active color',
    );
});

test('UI-1B implements Labels as a semantic pressed button', () => {
    const top_nav = ui_1b_clean_sources['src/components/layout/TopNavBar.tsx'];
    const label_toggle = jsx_button_blocks(top_nav).find((button) => /onClick=\{onToggleLabels\}/.test(button));

    assert.ok(label_toggle, 'Labels control must be a semantic button');
    assert.match(
        label_toggle,
        /aria-pressed=\{showLabels\}/,
        'Labels state must be exposed beyond its visual toggle',
    );
    assert.doesNotMatch(
        top_nav,
        /<div[^>]*onClick=\{onToggleLabels\}/,
        'Labels must not be implemented as a non-semantic clickable div',
    );
});

test('UI-1B exposes interaction mode selection without relying on color', () => {
    const top_nav = ui_1b_clean_sources['src/components/layout/TopNavBar.tsx'];

    assert.match(
        top_nav,
        /aria-pressed=\{active\}/,
        'interaction mode selection must be exposed beyond its active color',
    );
});

test('UI-1B keeps the macOS drag layer behind and outside interactive controls', () => {
    const top_nav = ui_1b_clean_sources['src/components/layout/TopNavBar.tsx'];

    assert.match(
        top_nav,
        /data-tauri-drag-region\s+className="absolute inset-0 z-0"/,
        'drag layer must remain behind controls',
    );
    for (const button of jsx_button_blocks(top_nav)) {
        assert.match(button, /data-tauri-drag-region="false"/, 'top-nav button must opt out of the drag region');
    }
    assert.match(top_nav, /onClick=\{onToggleLabels\}[\s\S]*data-tauri-drag-region="false"/);
});

test('UI-1B preserves viewportRef as the sole 3D pointer event target', () => {
    const app = ui_1b_clean_sources['src/App.tsx'];
    const shell = ui_1b_clean_sources['src/components/layout/Shell.tsx'];
    const camera_interaction = ui_1b_clean_sources['src/hooks/useCameraInteraction.ts'];

    assert.match(shell, /ref=\{viewportRef\}/);
    assert.match(app, /useCameraInteraction\(\s*\{\s*viewportRef,/);
    for (const [path, source] of Object.entries(ui_1b_clean_sources)) {
        if (path !== 'src/hooks/useCameraInteraction.ts') {
            assert_no_chrome_pointer_target(source, path);
        }
    }
    assert.throws(() => assert_no_chrome_pointer_target(
        '<div onPointerDown={capture} />',
        'synthetic chrome source',
    ));
    assert.match(camera_interaction, /const el = viewportRef\.current;/);
    assert.match(camera_interaction, /el\.addEventListener\('pointerdown', onPointerDown\)/);
    assert.match(camera_interaction, /el\.addEventListener\('pointermove', onPointerMove\)/);
    assert.match(camera_interaction, /el\.addEventListener\('pointerup', onPointerUp\)/);
});

test('UI-1B status fields retain textual meaning beyond color', () => {
    const bottom_status = ui_1b_clean_sources['src/components/layout/BottomStatusBar.tsx'];

    for (const label of ['SpaceGroup:', 'Volume:', 'Phonon Mode', 'Bonds:', 'Total Atoms:', 'Selected:']) {
        assert.match(bottom_status, new RegExp(label.replace(/[.*+?^\${}()|[\]\\]/g, '\\$&')));
    }
});

test('UI-1A rejects remote font loading and requires a local system font stack', () => {
    assert.doesNotMatch(index_css, /@import\s+url\s*\(/i);
    assert.doesNotMatch(index_css, /fonts\.googleapis\.com|fonts\.gstatic\.com/i);
    assert.match(index_css, /font-family\s*:[^;]*system-ui[^;]*-apple-system/i);
});

test('UI-1A requires complete opaque light and dark workbench tokens', () => {
    assert_complete_opaque_token_block(index_css, ':root');
    assert_complete_opaque_token_block(index_css, '.dark');
    for (const token of required_tokens) {
        assert.match(index_css, new RegExp(`${token}\\s*:`, 'i'), `missing ${token}`);
    }
    assert.match(index_css, /:root\s*\{[\s\S]*--cc-canvas\s*:/);
    assert.match(index_css, /\.dark\s*\{[\s\S]*--cc-canvas\s*:/);
    assert.match(index_css, /font-variant-numeric\s*:\s*tabular-nums/);
});

test('UI-1A token gate rejects incomplete and transparent dark themes', () => {
    const missing_token = index_css.replace('  --cc-danger: #f97066;\n', '');
    assert.throws(() => assert_complete_opaque_token_block(missing_token, '.dark'));

    for (const transparent_value of [
        '#17233180',
        '#1234',
        'rgba(23, 35, 49, 0.8)',
        'rgb(23 35 49 / 80%)',
        'transparent',
    ]) {
        const transparent_panel = index_css.replace(
            '  --cc-panel: #172331;',
            '  --cc-panel: ' + transparent_value + ';',
        );
        assert.throws(() => assert_complete_opaque_token_block(transparent_panel, '.dark'));
    }
});

test('UI-1A token gate accepts opaque CSS color forms', () => {
    for (const opaque_value of [
        '#172331',
        '#123',
        'rgb(23 35 49)',
        'hsl(210 36% 14%)',
        'rgba(23, 35, 49, 1)',
        'hsla(210, 36%, 14%, 100%)',
        'rgb(23 35 49 / 1)',
    ]) {
        assert.equal(is_opaque_css_color(opaque_value), true, opaque_value + ' must be accepted');
    }
});

test('UI-1A rejects persistent visual effects in every chrome, sidebar, and assistant surface', () => {
    const violations = [];
    for (const [path, source] of Object.entries(persistent_sources)) {
        for (const [effect, pattern] of forbidden_persistent_effects) {
            if (pattern.test(source)) violations.push(`${path}: ${effect}`);
        }
    }
    assert.deepEqual(violations, []);
});

test('UI-1A rejects new third-party UI component libraries', () => {
    const dependency_names = Object.keys({
        ...package_manifest.dependencies,
        ...package_manifest.devDependencies,
    });
    const forbidden_library = /^(?:@mui\/|@chakra-ui\/|@radix-ui\/|@headlessui\/|antd$|mantine$|prime(?:react|vue)$|semantic-ui-react$|react-bootstrap$)/;
    assert.equal(dependency_names.some((name) => forbidden_library.test(name)), false);
});

test('UI-1A rejects persistent decorative CSS animations in workbench surfaces', () => {
    for (const [path, source] of Object.entries(persistent_sources)) {
        assert.doesNotMatch(
            source,
            /\banimate-(?:pulse|ping|bounce)\b/,
            path + ' creates a persistent decorative CSS animation',
        );
    }
});

test('UI-1A rejects persistent animation loops in the workbench shell', () => {
    for (const [path, source] of Object.entries(persistent_sources)) {
        assert.doesNotMatch(source, /requestAnimationFrame|setInterval\s*\(/, `${path} creates a persistent animation loop`);
    }
});
