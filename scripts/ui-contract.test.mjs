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
