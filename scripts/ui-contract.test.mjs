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

const ui_1c_source_paths = [
    'src/App.tsx',
    'src/components/layout/LeftSidebar.tsx',
];

const ui_1c_sources = Object.fromEntries(await Promise.all(ui_1c_source_paths.map(async (path) => [
    path,
    await readFile(new URL('../' + path, import.meta.url), 'utf8'),
])));

const ui_1d_source_paths = [
    'src/components/layout/RightSidebar.tsx',
    'src/components/panels/index.ts',
    'src/components/panels/shared.tsx',
];

const ui_1d_sources = Object.fromEntries(await Promise.all(ui_1d_source_paths.map(async (path) => [
    path,
    await readFile(new URL('../' + path, import.meta.url), 'utf8'),
])));

const ui_1e_source_paths = [
    'src/components/panels/SupercellPanel.tsx',
    'src/components/panels/SlabPanel.tsx',
    'src/components/panels/AtomOperationsPanel.tsx',
    'src/components/panels/MeasurementPanel.tsx',
    'src/components/panels/shared.tsx',
];

const ui_1e_sources = Object.fromEntries(await Promise.all(ui_1e_source_paths.map(async (path) => [
    path,
    await readFile(new URL('../' + path, import.meta.url), 'utf8'),
])));

const ui_1f_source_paths = [
    'src/components/panels/BondAnalysisPanel.tsx',
    'src/components/panels/VolumetricPanel.tsx',
    'src/components/panels/PhononPanel.tsx',
    'src/components/panels/BrillouinZonePanel.tsx',
    'src/components/panels/WannierPanel.tsx',
    'src/components/panels/shared.tsx',
];

const ui_1f_sources = Object.fromEntries(await Promise.all(ui_1f_source_paths.map(async (path) => [
    path,
    await readFile(new URL('../' + path, import.meta.url), 'utf8'),
])));

const ui_1f_volumetric_backend_source = await readFile(
    new URL('../src-tauri/src/commands/volumetric.rs', import.meta.url),
    'utf8',
);

const ui_1g_source_paths = [
    'src/components/layout/PromptModal.tsx',
    'src/components/layout/PhononImportModal.tsx',
    'src/components/layout/SettingsModal.tsx',
    'src/components/layout/ExportImageModal.tsx',
];

const ui_1g_sources = Object.fromEntries(await Promise.all(ui_1g_source_paths.map(async (path) => [
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

const ui_1c_clean_sources = Object.fromEntries(Object.entries(ui_1c_sources).map(([path, source]) => [
    path,
    strip_comments(source),
]));

const ui_1d_clean_sources = Object.fromEntries(Object.entries(ui_1d_sources).map(([path, source]) => [
    path,
    strip_comments(source),
]));

const ui_1e_clean_sources = Object.fromEntries(Object.entries(ui_1e_sources).map(([path, source]) => [
    path,
    strip_comments(source),
]));

const ui_1f_clean_sources = Object.fromEntries(Object.entries(ui_1f_sources).map(([path, source]) => [
    path,
    strip_comments(source),
]));

const ui_1f_volumetric_backend_clean_source = strip_comments(ui_1f_volumetric_backend_source);

const ui_1g_clean_sources = Object.fromEntries(Object.entries(ui_1g_sources).map(([path, source]) => [
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

test('UI-1C collapses structure and atom content into one stable sidebar surface', () => {
    const left_sidebar = ui_1c_clean_sources['src/components/layout/LeftSidebar.tsx'];
    const sidebar_surface = left_sidebar.match(/<aside\b[\s\S]*?>/);

    assert.ok(sidebar_surface, 'left workspace must have one semantic sidebar surface');
    assert.match(sidebar_surface[0], /cc-panel/, 'sidebar surface must use the approved opaque panel primitive');
    assert.match(sidebar_surface[0], /data-sidebar-surface="structure-workspace"/);
    assert.match(left_sidebar, /data-sidebar-section="structure"/);
    assert.match(left_sidebar, /data-sidebar-section="atoms"/);
    assert.doesNotMatch(left_sidebar, /<Panel\b/, 'structure and atom content must not remain separate floating cards');
    assert.doesNotMatch(left_sidebar, /const Panel\b/, 'the old per-card surface component must be removed');
});

test('UI-1C makes lattice fields unit-explicit and numerically aligned', () => {
    const left_sidebar = ui_1c_clean_sources['src/components/layout/LeftSidebar.tsx'];
    const unit_cell_input = left_sidebar.slice(left_sidebar.indexOf('const UnitCellInput'));
    const atom_row = left_sidebar.slice(left_sidebar.indexOf('const AtomRow'));

    assert.match(unit_cell_input, /grid-cols-\[[^\]]*_auto\]/, 'lattice fields need a fixed unit column');
    assert.match(unit_cell_input, /\btabular-nums\b/, 'lattice values need tabular numerals');
    assert.match(unit_cell_input, /data-unit=\{unit\}/, 'lattice units must remain machine-visible');
    assert.match(unit_cell_input, /aria-label=\{`Lattice \$\{label\} \(\$\{unit\}\)`\}/);
    assert.equal(
        (atom_row.match(/\btabular-nums\b/g) || []).length,
        4,
        'fractional x/y/z and occupancy must each retain tabular numerals',
    );
});

test('UI-1C keeps intrinsic row and viewport selection on one App-owned path', () => {
    const app = ui_1c_clean_sources['src/App.tsx'];
    const left_sidebar = ui_1c_clean_sources['src/components/layout/LeftSidebar.tsx'];

    assert.match(app, /const \[selectedAtoms, setSelectedAtoms\] = useState<number\[\]>\(\[\]\)/);
    assert.match(app, /<LeftSidebar[\s\S]*selectedAtoms=\{selectedAtoms\}/);
    assert.match(app, /onSelectionChange=\{\(sel\) => \{[\s\S]*updateSelection\(sel\);[\s\S]*safeInvoke\('update_selection', \{ indices: sel \}\)/);
    assert.match(left_sidebar, /labels\.slice\(0, crystalState\.intrinsic_sites\)/);
    assert.match(left_sidebar, /isSelected=\{selectedAtoms\.includes\(i\)\}/);
    assert.match(left_sidebar, /onSelectionChange\(\[i\]\)/);
    assert.doesNotMatch(left_sidebar, /useState<\s*CrystalState/, 'sidebar must not introduce a second crystal-state owner');
});

test('UI-1C renders typed lattice failures instead of logging a false success', () => {
    const left_sidebar = ui_1c_clean_sources['src/components/layout/LeftSidebar.tsx'];

    assert.match(left_sidebar, /IpcException/);
    assert.match(left_sidebar, /const \[latticeError, setLatticeError\] = useState/);
    assert.match(left_sidebar, /await safeInvoke\('update_lattice_params'/);
    assert.match(left_sidebar, /error instanceof IpcException/);
    assert.match(left_sidebar, /setLatticeError\(\{ code: error\.code, message: error\.message \}\)/);
    assert.match(left_sidebar, /latticeError\.code/);
    assert.match(left_sidebar, /latticeError\.message/);
    assert.doesNotMatch(left_sidebar, /update_lattice_params[\s\S]*?catch\(console\.error\)/);
});

test('UI-1C makes empty and non-finite structure values explicitly non-physical', () => {
    const left_sidebar = ui_1c_clean_sources['src/components/layout/LeftSidebar.tsx'];

    assert.match(left_sidebar, /No structure loaded/);
    assert.match(left_sidebar, /Number\.isFinite\(volume\)/);
    assert.match(left_sidebar, /volume\.toFixed\(1\)\s*:\s*['"]—['"]/);
    assert.doesNotMatch(left_sidebar, /value=\{`\$\{vol\} Å³`\}/, 'empty values must not be presented as a physical zero-volume result');
});

test('UI-1D keeps one visible contextual inspector while retaining visited panels', () => {
    const right_sidebar = ui_1d_clean_sources['src/components/layout/RightSidebar.tsx'];

    assert.match(right_sidebar, /const activeTool\s*=/, 'the open tool must resolve to one active inspector');
    assert.match(right_sidebar, /PersistentInspector/, 'the inspector must own a persistent visited-panel boundary');
    assert.match(right_sidebar, /const \[hasOpened, setHasOpened\] = useState\(active\)/, 'unvisited panels must remain unmounted');
    assert.match(right_sidebar, /if \(!hasOpened\) return null;/, 'unvisited panels must not load their lazy chunk');
    assert.match(right_sidebar, /!active && "hidden"/, 'only the active contextual inspector may be visible');
    assert.doesNotMatch(right_sidebar, /<Accordion\b/, 'the old stacked accordion layout must not return');
});

test('UI-1D exposes every tool rail entry and preserves static overflow access', () => {
    const right_sidebar = ui_1d_clean_sources['src/components/layout/RightSidebar.tsx'];
    const required_tools = [
        'Structural Analysis',
        'Volumetric',
        'Phonon Animation',
        'Reciprocal Space',
        'Tight-Binding',
        'Supercell',
        'Cutting Plane',
        'Atom Operations',
        'Measurements',
    ];

    for (const tool of required_tools) {
        assert.match(right_sidebar, new RegExp(`key: '${tool}'`), 'missing tool rail entry: ' + tool);
    }
    assert.match(right_sidebar, /data-tool-rail="scientific-tools"/);
    assert.match(right_sidebar, /\boverflow-y-auto\b/, 'tool rail must scroll instead of shrinking icons');
    assert.match(right_sidebar, /\bmin-h-0\b/, 'tool rail must be bounded before it can scroll');
    assert.match(right_sidebar, /aria-label=\{section\.label\}/, 'each icon-only tool needs an accessible name');
    assert.match(right_sidebar, /aria-pressed=\{openAccordion === section\.key\}/, 'active inspector state must not rely on color');
});

test('UI-1D retains all nine lazy chunk boundaries without eager panel imports', () => {
    const right_sidebar = ui_1d_clean_sources['src/components/layout/RightSidebar.tsx'];
    const panel_index = ui_1d_clean_sources['src/components/panels/index.ts'];
    const lazy_panels = [
        'BondAnalysisPanel',
        'VolumetricPanel',
        'PhononPanel',
        'BrillouinZonePanel',
        'WannierPanel',
        'SupercellPanel',
        'SlabPanel',
        'AtomOperationsPanel',
        'MeasurementPanel',
    ];

    for (const panel of lazy_panels) {
        assert.match(panel_index, new RegExp(`${panel}: \\(\\) => import\\('./${panel}'\\)`));
        assert.match(right_sidebar, new RegExp(`lazy\\(lazyConfig\\.${panel}\\)`));
        assert.doesNotMatch(right_sidebar, new RegExp(`from ['"]\\.\\.\\/panels\\/${panel}['"]`));
    }
});

test('UI-1D deterministically enters and restores measurement mode', () => {
    const right_sidebar = ui_1d_clean_sources['src/components/layout/RightSidebar.tsx'];

    assert.match(right_sidebar, /previousModeRef/);
    assert.match(right_sidebar, /key === 'Measurements' && prev !== 'Measurements'/);
    assert.match(right_sidebar, /previousModeRef\.current = props\.interactionMode \|\| 'rotate'/);
    assert.match(right_sidebar, /props\.setInteractionMode\('measure'\)/);
    assert.match(right_sidebar, /prev === 'Measurements' && key !== 'Measurements'/);
    assert.match(right_sidebar, /props\.setInteractionMode\(previousModeRef\.current\)/);
});

test('UI-1D confines loading animation to the lazy fallback and keeps inspector surfaces plain', () => {
    const right_sidebar = ui_1d_clean_sources['src/components/layout/RightSidebar.tsx'];
    const shared = ui_1d_clean_sources['src/components/panels/shared.tsx'];

    assert.match(right_sidebar, /const fallbackSpinner[\s\S]*?\banimate-spin\b/);
    assert.equal((right_sidebar.match(/\banimate-[\w-]+\b/g) || []).length, 1, 'only the lazy fallback may animate');
    for (const [path, source] of [
        ['src/components/layout/RightSidebar.tsx', right_sidebar],
        ['src/components/panels/shared.tsx', shared],
    ]) {
        assert.doesNotMatch(source, /\bbackdrop-blur(?:-[\w\[\]/.-]+)?\b/, path + ' uses blur');
        assert.doesNotMatch(source, /\b(?:bg-)?gradient(?:-to-[\w-]+)?\b/, path + ' uses a gradient');
        assert.doesNotMatch(source, /\bshadow-(?:lg|xl|2xl)\b/, path + ' uses a large shadow');
    }
});

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

test('UI-1E preserves structural-edit command names and complete payloads', () => {
    const supercell = ui_1e_clean_sources['src/components/panels/SupercellPanel.tsx'];
    const slab = ui_1e_clean_sources['src/components/panels/SlabPanel.tsx'];
    const atom_operations = ui_1e_clean_sources['src/components/panels/AtomOperationsPanel.tsx'];
    const measurement = ui_1e_clean_sources['src/components/panels/MeasurementPanel.tsx'];

    assert.match(supercell, /safeInvoke\('apply_supercell',\s*\{\s*matrix\s*\}\)/);
    assert.match(supercell, /safeInvoke\('restore_unitcell'\)/);
    assert.match(slab, /safeInvoke\('apply_slab',\s*\{\s*miller:\s*\[slab\.h,\s*slab\.k,\s*slab\.l\],\s*layers:\s*slab\.layers,\s*vacuumA:\s*slab\.vacuum\s*\}\)/);
    assert.match(atom_operations, /safeInvoke\('delete_atoms',\s*\{\s*indices:\s*selectedAtoms\s*\}\)/);
    assert.match(atom_operations, /safeInvoke\('substitute_atoms',\s*\{\s*indices:\s*selectedAtoms,\s*newElementSymbol:\s*newElem\.trim\(\),\s*newAtomicNumber:\s*0\s*\}\)/);
    assert.match(measurement, /safeInvoke\('clear_measurements'\)/);
    assert.match(measurement, /safeInvoke\('add_measurement',\s*\{\s*indices:\s*selectedAtoms\s*\}\)/);
});

test('UI-1E makes shared structural controls focusable and exposes disabled, busy, invalid, and error states', () => {
    const shared = ui_1e_clean_sources['src/components/panels/shared.tsx'];

    assert.match(shared, /data-ui-control="field"/, 'shared field primitive needs a stable semantic outlet');
    assert.match(shared, /data-ui-control="action"/, 'shared action primitive needs a stable semantic outlet');
    assert.match(shared, /focus-visible:/, 'keyboard focus must not depend on hover or color alone');
    assert.match(shared, /aria-busy=/, 'pending work must be exposed semantically');
    assert.match(shared, /aria-invalid=/, 'invalid input must be exposed semantically');
    assert.match(shared, /role="alert"/, 'visible mutation failures need an assertive semantic outlet');
    assert.match(shared, /error\.code/, 'error policy must branch on the typed code');
    assert.match(shared, /error\.message/, 'error presentation must preserve the typed message');
});

test('UI-1E routes structural mutation failures through typed error state without changing measurement lifecycle', () => {
    const measurement = ui_1e_clean_sources['src/components/panels/MeasurementPanel.tsx'];
    const structural_panels = [
        'src/components/panels/SupercellPanel.tsx',
        'src/components/panels/SlabPanel.tsx',
        'src/components/panels/AtomOperationsPanel.tsx',
        'src/components/panels/MeasurementPanel.tsx',
    ];

    for (const path of structural_panels) {
        const source = ui_1e_clean_sources[path];
        assert.match(source, /IpcException/, path + ' must retain typed IPC failures');
        assert.match(source, /set(?:[A-Za-z]+)?Error\(/, path + ' must retain visible local error state');
        assert.match(source, /error\.message|\.message\s*\}/, path + ' must render the backend message without replacement');
    }
    assert.match(measurement, /selectedAtoms\.length\s*>=\s*2\s*&&\s*selectedAtoms\.length\s*<=\s*4/);
    assert.match(measurement, /onSelectionChange\(\[\]\)/, 'successful measurement still clears only the local selection projection');
});

test('UI-1E assigns busy state only to the operation that owns the active mutation', () => {
    const operation_specific_busy = [
        ['src/components/panels/SupercellPanel.tsx', ui_1e_clean_sources['src/components/panels/SupercellPanel.tsx'], [
            ['Execute Supercell', 'supercell'],
            ['Restore Original Cell', 'restore'],
        ]],
        ['src/components/panels/SlabPanel.tsx', ui_1e_clean_sources['src/components/panels/SlabPanel.tsx'], [
            ['Cut', 'cut'],
            ['Reset', 'reset'],
        ]],
        ['src/components/panels/AtomOperationsPanel.tsx', ui_1e_clean_sources['src/components/panels/AtomOperationsPanel.tsx'], [
            ['Replace Atom(s)', 'replace'],
            ['Delete Atom(s)', 'delete'],
        ]],
        ['src/components/panels/MeasurementPanel.tsx', ui_1e_clean_sources['src/components/panels/MeasurementPanel.tsx'], [
            ['Clear All Measurements', 'clear'],
            ['Add Measurement from Selection', 'add'],
        ]],
    ];

    for (const [path, source, actions] of operation_specific_busy) {
        assert.match(source, /const \[activeOperation, setActiveOperation\] = useState</, path + ' needs a discriminated operation state');
        assert.match(source, /if \([^)]*activeOperation[^)]*\) return;/, path + ' must reject overlapping mutation requests');
        assert.doesNotMatch(source, /busy=\{isBusy\}/, path + ' must not mark an unrelated action as busy');
        const action_buttons = source.match(/<ActionButton\b[^>]*\/>/g) || [];
        for (const [label, operation] of actions) {
            const action_button = action_buttons.find((button) => button.includes(`label="${label}"`));
            assert.ok(action_button, path + ' is missing the ' + label + ' action');
            assert.match(
                action_button,
                new RegExp(`busy=\\{activeOperation === '${operation}'\\}`),
                path + ' must expose busy only for ' + operation,
            );
        }
    }

    const swapped_busy = ui_1e_clean_sources['src/components/panels/AtomOperationsPanel.tsx']
        .replace("busy={activeOperation === 'replace'}", 'busy={isBusy}');
    assert.throws(() => assert.doesNotMatch(swapped_busy, /busy=\{isBusy\}/));
});

test('UI-1E rejects raw slab range controls and requires complete range semantics', () => {
    const shared = ui_1e_clean_sources['src/components/panels/shared.tsx'];
    const slab = ui_1e_clean_sources['src/components/panels/SlabPanel.tsx'];
    const supercell = ui_1e_clean_sources['src/components/panels/SupercellPanel.tsx'];

    assert.match(shared, /export const RangeInput/);
    assert.match(shared, /data-ui-control="range"/);
    assert.match(shared, /type="range"/);
    assert.match(shared, /aria-busy=\{busy\}/);
    assert.match(shared, /aria-invalid=\{invalid\}/);
    assert.match(shared, /focus-visible:ring-1/);
    assert.match(slab, /import \{ ActionButton, NumberInput, PanelError, RangeInput \} from '\.\/shared';/);
    assert.equal((slab.match(/<RangeInput\b/g) || []).length, 2, 'Layers and Vacuum must use the shared range primitive');
    assert.doesNotMatch(slab, /<input\s+type="range"/, 'Slab must not reintroduce an unstyled raw range input');
    assert.match(slab, /invalid=\{invalidMiller\}/, 'all-zero Miller input must be exposed as invalid before submission');
    for (const axis of ['nx', 'ny', 'nz']) {
        assert.match(supercell, new RegExp(`invalid=\\{sc\\.${axis} < 1\\}`), 'supercell ' + axis + ' must expose a non-positive factor as invalid');
    }

    const raw_range_regression = slab.replace('<RangeInput label="Layers"', '<input type="range"');
    assert.throws(() => assert.doesNotMatch(raw_range_regression, /<input\s+type="range"/));
});

test('UI-1F preserves scientific symbols and the existing renderer/listener ownership boundaries', () => {
    const bond = ui_1f_clean_sources['src/components/panels/BondAnalysisPanel.tsx'];
    const volumetric = ui_1f_clean_sources['src/components/panels/VolumetricPanel.tsx'];
    const phonon = ui_1f_clean_sources['src/components/panels/PhononPanel.tsx'];
    const brillouin_zone = ui_1f_clean_sources['src/components/panels/BrillouinZonePanel.tsx'];
    const wannier = ui_1f_clean_sources['src/components/panels/WannierPanel.tsx'];

    assert.match(bond, /Å/);
    assert.match(phonon, /q\s*=\s*\(/);
    assert.match(phonon, /cm⁻¹/);
    assert.match(brillouin_zone, /N<sub>k<\/sub>/);
    assert.match(wannier, /R\s*=\s*\[/);
    assert.match(wannier, /\|t\|/);
    assert.equal((volumetric.match(/safeListen\(/g) || []).length, 1, 'volumetric panel must keep one event owner');
    assert.match(volumetric, /safeListen\('volumetric_loaded'/);
    assert.match(volumetric, /return\s*\(\)\s*=>\s*\{\s*unlisten\(\);\s*\}/);
});

test('UI-1F gives every async scientific panel explicit busy, unavailable, and typed-error states', () => {
    const scientific_panels = [
        'src/components/panels/BondAnalysisPanel.tsx',
        'src/components/panels/VolumetricPanel.tsx',
        'src/components/panels/PhononPanel.tsx',
        'src/components/panels/BrillouinZonePanel.tsx',
        'src/components/panels/WannierPanel.tsx',
    ];

    for (const path of scientific_panels) {
        const source = ui_1f_clean_sources[path];
        assert.match(source, /useState\(false\)/, path + ' needs a real asynchronous busy state');
        assert.match(source, /aria-busy=/, path + ' must expose pending work semantically');
        assert.match(source, /role="status"/, path + ' needs an explicit unavailable/empty state');
        assert.match(source, /role="alert"|<PanelError\b/, path + ' needs a visible error state');
        assert.match(source, /IpcException/, path + ' must preserve typed IPC failures');
        assert.match(source, /error\.message|\.message\s*\}/, path + ' must retain the backend error message');
        assert.match(source, /disabled=\{[^}]*Busy|disabled=\{[^}]*Loading/, path + ' must prevent duplicate work while pending');
    }
});

test('UI-1F rejects fabricated volumetric and hopping range controls', () => {
    const volumetric = ui_1f_clean_sources['src/components/panels/VolumetricPanel.tsx'];
    const wannier = ui_1f_clean_sources['src/components/panels/WannierPanel.tsx'];

    assert.match(volumetric, /const hasLoadedVolumetricData = volumetricInfo !== null;/, 'volumetric controls need an explicit loaded-data guard');
    assert.match(volumetric, /const hasUsableVolumetricRange = Number\.isFinite\([^)]*data_min[^)]*\)[\s\S]*Number\.isFinite\([^)]*data_max[^)]*\)[\s\S]*> 0/, 'zero, NaN, and infinite volumetric ranges must be rejected');
    assert.match(volumetric, /hasLoadedVolumetricData && \(/, 'volumetric render controls must not mount before data loads');
    assert.match(volumetric, /hasUsableVolumetricRange \?/, 'invalid volumetric range controls need an explicit unavailable branch');
    assert.doesNotMatch(volumetric, /<input\s+type="range"/, 'volumetric controls must use the shared range primitive');

    assert.match(wannier, /const hasUsableHoppingRange = Number\.isFinite\(wannierInfo\.t_max\) && wannierInfo\.t_max > 0/, 'zero, NaN, and infinite hopping ranges must be rejected');
    assert.match(wannier, /hasUsableHoppingRange \?/, 'invalid hopping thresholds need an explicit unavailable branch');
    assert.doesNotMatch(wannier, /<input\s+type="range"/, 'Wannier thresholds must use the shared range primitive');

    const fabricated_volume_controls = volumetric
        .replace('const hasLoadedVolumetricData = volumetricInfo !== null;', 'const hasLoadedVolumetricData = true;')
        .replace('const hasUsableVolumetricRange', 'const hasUsableVolumetricRangeBroken');
    assert.throws(() => assert.match(fabricated_volume_controls, /const hasUsableVolumetricRange =/));
});

test('UI-1F converges scientific panels on shared controls and visible typed failures', () => {
    const expected_primitives = {
        'src/components/panels/BondAnalysisPanel.tsx': ['ActionButton', 'PanelError'],
        'src/components/panels/VolumetricPanel.tsx': ['ActionButton', 'RangeInput', 'PanelError'],
        'src/components/panels/PhononPanel.tsx': ['ActionButton', 'RangeInput', 'PanelError'],
        'src/components/panels/BrillouinZonePanel.tsx': ['ActionButton', 'PanelError'],
        'src/components/panels/WannierPanel.tsx': ['ActionButton', 'RangeInput', 'PanelError'],
    };

    for (const [path, primitives] of Object.entries(expected_primitives)) {
        const source = ui_1f_clean_sources[path];
        assert.match(source, /from '\.\/shared';/, path + ' must import the UI-1D shared primitives');
        for (const primitive of primitives) {
            assert.match(source, new RegExp(`<${primitive}\\b`), path + ' must render ' + primitive);
        }
    }

    const volumetric = ui_1f_clean_sources['src/components/panels/VolumetricPanel.tsx'];
    const wannier = ui_1f_clean_sources['src/components/panels/WannierPanel.tsx'];
    assert.doesNotMatch(volumetric, /alert\(String\(/, 'volumetric renderer failures must remain in the panel error surface');
    assert.doesNotMatch(wannier, /catch\(console\.error\)/, 'Wannier renderer failures must remain in the panel error surface');
    assert.match(volumetric, /setPanelError\(/, 'volumetric renderer failures must retain typed errors');
    assert.match(wannier, /setPanelError\(/, 'Wannier renderer failures must retain typed errors');
});

test('UI-1F keeps operation-specific busy labels without changing renderer command ownership', () => {
    const bond = ui_1f_clean_sources['src/components/panels/BondAnalysisPanel.tsx'];
    const volumetric = ui_1f_clean_sources['src/components/panels/VolumetricPanel.tsx'];
    const phonon = ui_1f_clean_sources['src/components/panels/PhononPanel.tsx'];
    const brillouin_zone = ui_1f_clean_sources['src/components/panels/BrillouinZonePanel.tsx'];
    const wannier = ui_1f_clean_sources['src/components/panels/WannierPanel.tsx'];

    assert.match(wannier, /useState<'load' \| 'clear' \| null>/, 'Wannier needs operation-specific busy ownership');
    assert.match(wannier, /label="Load wannier90_hr\.dat\.\.\."[\s\S]*busy=\{activeOperation === 'load'\}/, 'load must expose only its own busy state');
    assert.match(wannier, /label="Clear Wannier Overlay"[\s\S]*busy=\{activeOperation === 'clear'\}/, 'clear must expose only its own busy state');

    const expected_command_counts = [
        [bond, 'get_bond_analysis', 1],
        [volumetric, 'load_volumetric_file', 1],
        [volumetric, 'set_isovalue', 2],
        [volumetric, 'set_volume_render_mode', 2],
        [volumetric, 'set_isosurface_sign_mode', 2],
        [volumetric, 'set_isosurface_opacity', 1],
        [volumetric, 'set_volume_colormap', 1],
        [volumetric, 'set_volume_density_cutoff', 1],
        [volumetric, 'set_volume_opacity_range', 1],
        [phonon, 'load_axsf_phonon', 1],
        [phonon, 'load_phonon_interactive', 1],
        [phonon, 'set_phonon_mode', 1],
        [brillouin_zone, 'get_bz_label_positions', 1],
        [brillouin_zone, 'compute_brillouin_zone', 1],
        [brillouin_zone, 'toggle_bz_display', 2],
        [brillouin_zone, 'generate_kpath_text', 1],
        [brillouin_zone, 'write_text_file', 1],
        [wannier, 'load_wannier_hr', 1],
        [wannier, 'clear_wannier', 1],
        [wannier, 'set_wannier_t_min', 1],
        [wannier, 'set_wannier_orbital', 1],
        [wannier, 'toggle_wannier_onsite', 1],
        [wannier, 'set_wannier_r_shell', 1],
        [wannier, 'toggle_hopping_display', 1],
    ];

    for (const [source, command, expected] of expected_command_counts) {
        assert.equal((source.match(new RegExp(`safeInvoke\\('${command}'`, 'g')) || []).length, expected, command + ' command ownership changed');
    }
});

test('UI-1F rejects positive finite ranges whose derived step underflows to zero', () => {
    const volumetric = ui_1f_clean_sources['src/components/panels/VolumetricPanel.tsx'];
    const wannier = ui_1f_clean_sources['src/components/panels/WannierPanel.tsx'];

    assert.equal(Number.MIN_VALUE > 0, true);
    assert.equal(Number.isFinite(Number.MIN_VALUE), true);
    assert.equal(Number.MIN_VALUE / 1000, 0, 'the smallest finite positive volume bound underflows at the current divisor');
    assert.equal(Number.MIN_VALUE / 100, 0, 'the smallest finite positive hopping bound underflows at the current divisor');

    assert.match(volumetric, /const isovalueStep = volumetricBound \/ 1000;/);
    assert.match(volumetric, /const densityCutoffStep = volumetricBound \/ 500;/);
    assert.match(volumetric, /Number\.isFinite\(isovalueStep\)[\s\S]*isovalueStep > 0/, 'isovalue step must be checked after division');
    assert.match(volumetric, /Number\.isFinite\(densityCutoffStep\)[\s\S]*densityCutoffStep > 0/, 'density step must be checked after division');
    assert.match(volumetric, /step=\{isovalueStep\}/);
    assert.match(volumetric, /step=\{densityCutoffStep\}/);

    assert.match(wannier, /const hoppingStep = wannierInfo\.t_max \/ 100;/);
    assert.match(wannier, /Number\.isFinite\(hoppingStep\)[\s\S]*hoppingStep > 0/, 'hopping step must be checked after division');
    assert.match(wannier, /step=\{hoppingStep\}/);
});

test('UI-1F commits discrete renderer state only after its IPC request is issued', () => {
    const volumetric = ui_1f_clean_sources['src/components/panels/VolumetricPanel.tsx'];
    const phonon = ui_1f_clean_sources['src/components/panels/PhononPanel.tsx'];
    const wannier = ui_1f_clean_sources['src/components/panels/WannierPanel.tsx'];

    const assert_command_precedes_state = (source, command, state_update, occurrence = 1) => {
        const command_token = `safeInvoke('${command}'`;
        let command_index = -1;
        for (let index = 0; index < occurrence; index++) {
            command_index = source.indexOf(command_token, command_index + 1);
        }
        const state_index = source.indexOf(state_update);
        assert.ok(command_index >= 0, command + ' command is missing');
        assert.ok(state_index > command_index, state_update + ' must not commit before ' + command);
    };

    assert_command_precedes_state(phonon, 'set_phonon_mode', 'setActiveModeIdx(idx)');
    const mode_handler_start = phonon.indexOf('const handle_select_mode');
    const mode_handler_end = phonon.indexOf('const handle_toggle_animation', mode_handler_start);
    assert.ok(mode_handler_start >= 0 && mode_handler_end > mode_handler_start,
        'phonon panel must retain a dedicated mode-selection handler');
    const mode_handler = phonon.slice(mode_handler_start, mode_handler_end);
    const mode_guard = mode_handler.search(/if\s*\([^)]*\)\s*return;/);
    const mode_invoke = mode_handler.indexOf("safeInvoke('set_phonon_mode'");
    assert.ok(mode_guard >= 0 && mode_guard < mode_invoke,
        'phonon mode selection must reject an overlapping control operation before issuing IPC');
    const mode_select_start = phonon.indexOf('<SelectInput');
    const mode_select_end = phonon.indexOf('</SelectInput>', mode_select_start);
    assert.ok(mode_select_start >= 0 && mode_select_end > mode_select_start,
        'phonon panel must retain its mode selector');
    assert.match(phonon.slice(mode_select_start, mode_select_end), /disabled=\{[^}]+\}/,
        'phonon mode selector must be disabled while a control operation is pending');

    assert_command_precedes_state(volumetric, 'set_volume_render_mode', 'setVolumeRenderMode(mode)', 2);
    assert_command_precedes_state(volumetric, 'set_isosurface_sign_mode', 'setSignMode(mode)', 2);
    assert_command_precedes_state(volumetric, 'set_volume_colormap', 'setVolumeColormap(mode)');
    assert.match(volumetric, /pendingControl/, 'volume selects must reject overlapping requests');

    assert_command_precedes_state(wannier, 'set_wannier_orbital', 'setActiveOrbitals(next)');
    assert_command_precedes_state(wannier, 'toggle_wannier_onsite', 'setShowOnSite(checked)');
    assert_command_precedes_state(wannier, 'set_wannier_r_shell', 'setActiveRShells(next)');
    assert_command_precedes_state(wannier, 'toggle_hopping_display', 'setIsWannierVisible(next)');
    assert.match(wannier, /pendingControl/, 'Wannier discrete controls must reject overlapping requests');
});

test('UI-1F keeps scientific controls and information surfaces on opaque shared tokens', () => {
    const shared = ui_1f_clean_sources['src/components/panels/shared.tsx'];
    const select_panels = [
        'src/components/panels/VolumetricPanel.tsx',
        'src/components/panels/PhononPanel.tsx',
        'src/components/panels/BrillouinZonePanel.tsx',
    ];

    assert.match(shared, /export const SelectInput/);
    assert.match(shared, /data-ui-control="select"/);
    assert.match(shared, /bg-\[var\(--cc-field\)\]/);
    assert.match(shared, /border-\[var\(--cc-border\)\]/);

    for (const path of select_panels) {
        const source = ui_1f_clean_sources[path];
        assert.match(source, /<SelectInput\b/, path + ' must use the shared select primitive');
        assert.doesNotMatch(source, /<select\b/, path + ' must not keep a raw select');
    }

    for (const [path, original_source] of Object.entries(ui_1f_clean_sources)) {
        if (path.endsWith('/shared.tsx')) continue;
        const source = original_source.replace(/<div className="bg-slate-900[\s\S]*?<\/div>/g, '');
        assert.doesNotMatch(source, /\b(?:dark:)?(?:bg|text|border)-slate-[\w/.-]+\b/, path + ' retains a legacy slate presentation surface');
    }
});

test('UI-1F gives ellipsis-labelled actions an explicit non-duplicated busy label', () => {
    const shared = ui_1f_clean_sources['src/components/panels/shared.tsx'];
    assert.match(shared, /busyLabel\?: string/);
    assert.match(shared, /busyLabel \?\?/);

    for (const [path, source] of Object.entries(ui_1f_clean_sources)) {
        const action_buttons = source.match(/<ActionButton\b[\s\S]*?\/>/g) || [];
        for (const action_button of action_buttons) {
            if (!/label="[^"]*\.\.\."/.test(action_button) || !/busy=/.test(action_button)) continue;
            assert.match(action_button, /busyLabel="[^"]+"/, path + ' needs an explicit busy label for an ellipsis-labelled action');
            assert.doesNotMatch(action_button, /busyLabel="[^"]*\.\.\.[^"]*…[^"]*"/, path + ' duplicates ASCII and Unicode ellipses');
        }
    }
});

test('UI-1F retains its five lazy chunks and frame/listener boundaries during repair', () => {
    const panel_index = ui_1d_clean_sources['src/components/panels/index.ts'];
    const volumetric = ui_1f_clean_sources['src/components/panels/VolumetricPanel.tsx'];
    const phonon = ui_1f_clean_sources['src/components/panels/PhononPanel.tsx'];
    const scientific_panels = ['BondAnalysisPanel', 'VolumetricPanel', 'PhononPanel', 'BrillouinZonePanel', 'WannierPanel'];

    for (const panel of scientific_panels) {
        assert.match(panel_index, new RegExp(`${panel}: \\(\\) => import\\('\\./${panel}'\\)`));
        assert.doesNotMatch(panel_index, new RegExp(`import\\s+${panel}\\s+from`));
    }
    assert.equal((volumetric.match(/safeListen\(/g) || []).length, 1);
    assert.match(volumetric, /return\s*\(\)\s*=>\s*\{\s*unlisten\(\);\s*\}/);
});

test('UI-1F mounts VolumetricPanel in the persistent inspector tree', () => {
    const sidebar = ui_1d_clean_sources['src/components/layout/RightSidebar.tsx'];
    const outer_return = sidebar.indexOf('\n    return (');
    const volumetric_mount = sidebar.indexOf('<VolumetricPanel');

    assert.ok(outer_return >= 0, 'RightSidebar must have a render return');
    assert.ok(
        volumetric_mount > outer_return,
        'VolumetricPanel must be mounted by the persistent inspector tree, not returned only by an active-tool switch',
    );
    assert.doesNotMatch(
        sidebar,
        /case 'Volumetric':\s*return <VolumetricPanel/,
        'switching tools must not unmount the sole volumetric event owner',
    );
});

test('UI-1F does not unmount scientific panel state when the inspector closes', () => {
    const sidebar = ui_1d_clean_sources['src/components/layout/RightSidebar.tsx'];

    assert.doesNotMatch(
        sidebar,
        /\{activeTool && <Suspense/,
        'closing the inspector must not discard mounted scientific panel state and listeners',
    );
});

test('UI-1F mirrors the initial Renderer-derived density cutoff after volumetric load', () => {
    const volumetric = ui_1f_clean_sources['src/components/panels/VolumetricPanel.tsx'];
    const listener = volumetric.slice(
        volumetric.indexOf("safeListen('volumetric_loaded'"),
        volumetric.indexOf('const handleRenderMode'),
    );

    assert.match(
        listener,
        /safeInvoke\('set_isovalue',[\s\S]*?(?:\.then\([\s\S]*?setDensityCutoff\(defaultIsovalue\)|await[\s\S]*?setDensityCutoff\(defaultIsovalue\))/,
        'initial isovalue must mirror the cutoff that the Renderer derives in Both mode',
    );
});

test('UI-1F mirrors the Renderer density cutoff after render-mode completion', () => {
    const volumetric = ui_1f_clean_sources['src/components/panels/VolumetricPanel.tsx'];
    const render_mode_handler = volumetric.slice(
        volumetric.indexOf('const handleRenderMode'),
        volumetric.indexOf('const handleSignMode'),
    );

    assert.match(
        render_mode_handler,
        /await safeInvoke\('set_volume_render_mode',[\s\S]*?setVolumeRenderMode\(mode\)[\s\S]*?setDensityCutoff\(mode === 'both' \? isovalue : 0\)/,
        'render-mode completion must mirror the Renderer cutoff reset before controls re-enable',
    );
});

test('UI-1F mirrors the Renderer density cutoff after isovalue completion', () => {
    const volumetric = ui_1f_clean_sources['src/components/panels/VolumetricPanel.tsx'];
    const isovalue_control = volumetric.slice(
        volumetric.indexOf('<RangeInput label="Isovalue"'),
        volumetric.indexOf('<RangeInput label="Surface Opacity"'),
    );

    assert.match(
        isovalue_control,
        /safeInvoke\('set_isovalue',[\s\S]*?(?:\.then\([\s\S]*?setDensityCutoff\(value\)|await[\s\S]*?setDensityCutoff\(value\))/,
        'isovalue completion must mirror the coupled density cutoff without an extra IPC command',
    );
});

test('UI-1F reinitializes controlled volumetric mirrors for every loaded dataset', () => {
    const volumetric = ui_1f_clean_sources['src/components/panels/VolumetricPanel.tsx'];
    const apply_info = volumetric.slice(
        volumetric.indexOf('const applyVolumetricInfo'),
        volumetric.indexOf('const hasLoadedVolumetricData'),
    );

    assert.match(apply_info, /setVolumetricInfo\(info\)[\s\S]*?setSurfaceOpacity\(0\.5\)/,
        'a reload must not retain the previous dataset surface opacity');
    assert.match(apply_info, /setVolumetricInfo\(info\)[\s\S]*?setDensityCutoff\(0\)/,
        'a reload must not retain the previous dataset density cutoff');
    assert.match(apply_info, /setVolumetricInfo\(info\)[\s\S]*?setOpacityScale\(1\)/,
        'a reload must not retain the previous dataset volume opacity scale');
    assert.match(apply_info, /setVolumetricInfo\(info\)[\s\S]*?setVolumeRenderMode\('both'\)/,
        'a reload must mirror the renderer default render mode');
    assert.match(
        apply_info,
        /setVolumeColormap\(info\.data_min < -0\.01 \* Math\.abs\(info\.data_max\) \? 'coolwarm' : 'viridis'\)/,
        'a reload must mirror the backend signed-data colormap decision',
    );
});

test('UI-1F does not claim Both sign mode before its renderer IPC succeeds', () => {
    const volumetric = ui_1f_clean_sources['src/components/panels/VolumetricPanel.tsx'];
    const apply_info = volumetric.slice(
        volumetric.indexOf('const applyVolumetricInfo'),
        volumetric.indexOf('const hasLoadedVolumetricData'),
    );
    const listener = volumetric.slice(
        volumetric.indexOf("safeListen('volumetric_loaded'"),
        volumetric.indexOf('const handleRenderMode'),
    );

    assert.match(
        apply_info,
        /setSignMode\('positive'\)/,
        'a newly loaded isosurface must initially mirror the Renderer sign_mode=0 default',
    );
    assert.doesNotMatch(
        apply_info,
        /setSignMode\('both'\)/,
        'Both must not be committed locally before set_isosurface_sign_mode succeeds',
    );
    assert.match(
        listener,
        /safeInvoke\('set_isosurface_sign_mode',\s*\{\s*mode:\s*'both'\s*\}\)[\s\S]*?\.then\(\(\) => setSignMode\('both'\)\)[\s\S]*?\.catch\(/,
        'an IPC rejection must leave the local mirror at the loaded Renderer default',
    );
});

test('UI-1F clears a signed dataset colormap before an unsigned volumetric reload', () => {
    const signed_data = { data_min: -1, data_max: 1 };
    const unsigned_data = { data_min: 0, data_max: 1 };
    const is_signed = ({ data_min, data_max }) => data_min < -0.01 * Math.abs(data_max);

    assert.equal(is_signed(signed_data), true, 'the predecessor dataset selects Coolwarm mode 4');
    assert.equal(is_signed(unsigned_data), false, 'the replacement dataset must select Viridis mode 0');

    const load_colormap = ui_1f_volumetric_backend_clean_source.slice(
        ui_1f_volumetric_backend_clean_source.indexOf('let has_negative'),
        ui_1f_volumetric_backend_clean_source.indexOf('new_state.volumetric_data = Some(vol_data)'),
    );
    const resets_with_else = /if has_negative\s*\{[\s\S]*?active_colormap_mode\s*=\s*4;[\s\S]*?\}\s*else\s*\{[\s\S]*?active_colormap_mode\s*=\s*0;/.test(load_colormap);
    const assigns_both_modes = /active_colormap_mode\s*=\s*if has_negative\s*\{\s*4\s*\}\s*else\s*\{\s*0\s*\};/.test(load_colormap);

    assert.ok(
        resets_with_else || assigns_both_modes,
        'signed → unsigned reload must explicitly reset Renderer active_colormap_mode to Viridis',
    );
});

test('UI-1F resets the Wannier threshold mirror on every successful load', () => {
    const wannier = ui_1f_clean_sources['src/components/panels/WannierPanel.tsx'];
    const load_handler = wannier.slice(
        wannier.indexOf('const handle_load_wannier'),
        wannier.indexOf('const handle_clear_wannier'),
    );

    assert.ok(0 < 0.01, 'the reset threshold must remain a positive physical cutoff');
    assert.match(
        load_handler,
        /await safeInvoke\('load_wannier_hr',[\s\S]*?setWannierInfo\(info\)[\s\S]*?setTMin\(0\.01\)/,
        'a new overlay must not inherit the previous file threshold',
    );
});

test('UI-1F refuses a fake Wannier threshold when 0 < t_max < 0.01', () => {
    const wannier = ui_1f_clean_sources['src/components/panels/WannierPanel.tsx'];

    assert.equal(0.005 > 0, true, 'the counterexample has a positive declared maximum');
    assert.equal(0.01 > 0.005, true, 'the backend default threshold exceeds the declared maximum');
    assert.match(
        wannier,
        /const hasUsableHoppingThreshold = hasUsableHoppingStep && tMin <= wannierInfo\.t_max;/,
        'the UI must only expose a threshold the backend can actually retain',
    );
    assert.match(wannier, /hasUsableHoppingThreshold \? \(/,
        'an unavailable threshold must render an explicit unavailable state');
    assert.doesNotMatch(wannier, /Math\.min\(tMin,\s*wannierInfo\.t_max\)/,
        'clamping the displayed value would falsely claim the backend accepted it');
});

test('UI-1F resets the Wannier on-site mirror on every successful load', () => {
    const wannier = ui_1f_clean_sources['src/components/panels/WannierPanel.tsx'];
    const load_handler = wannier.slice(
        wannier.indexOf('const handle_load_wannier'),
        wannier.indexOf('const handle_clear_wannier'),
    );

    assert.match(
        load_handler,
        /await safeInvoke\('load_wannier_hr',[\s\S]*?setActiveOrbitals\([\s\S]*?setShowOnSite\(false\)/,
        'a new overlay must not inherit the previous file on-site visibility',
    );
});

test('UI-1G preserves file cancel and modal command contracts before presentation migration', () => {
    const phonon_import = ui_1g_clean_sources['src/components/layout/PhononImportModal.tsx'];
    const settings = ui_1g_clean_sources['src/components/layout/SettingsModal.tsx'];
    const export_image = ui_1g_clean_sources['src/components/layout/ExportImageModal.tsx'];
    const phonon_panel = ui_1f_clean_sources['src/components/panels/PhononPanel.tsx'];

    assert.match(phonon_import, /safeDialogOpen\(/);
    assert.match(phonon_import, /if\s*\(path\s*&&\s*typeof path === ['"]string['"]\)/, 'cancelled import picker must not submit a path');
    assert.match(export_image, /(?:const|let) path = await safeDialogSave\(/);
    assert.match(export_image, /if\s*\(!path\)\s*return;/, 'cancelled export picker must not invoke export');
    assert.match(export_image, /safeInvoke\('export_image',\s*\{\s*path,\s*width:\s*outputW,\s*height:\s*outputH,\s*bgMode,\s*\}\)/);
    assert.match(settings, /safeInvoke\('get_settings'\)/);
    assert.match(settings, /safeInvoke\('update_settings',\s*\{\s*newSettings:\s*settings\s*\}\)/);
    assert.match(phonon_panel, /safeInvoke\('load_axsf_phonon',\s*\{\s*path:\s*paths\.axsf\s*\}\)/);
    assert.match(phonon_panel, /safeInvoke\('load_phonon_interactive',\s*\{\s*scfIn:\s*paths\.scfIn,\s*scfOut:\s*paths\.scfOut,\s*modes:\s*paths\.modes\s*\}\)/);
});

test('UI-1G implements complete blocking-dialog semantics for every modal', () => {
    for (const [path, source] of Object.entries(ui_1g_clean_sources)) {
        assert.match(source, /role="dialog"/, path + ' must use dialog semantics');
        assert.match(source, /aria-modal="true"/, path + ' must declare its blocking scope');
        assert.match(source, /aria-labelledby=/, path + ' needs an accessible dialog title');
        assert.match(source, /keydown/, path + ' must listen for Escape only while mounted');
        assert.match(source, /(?:event|e)\.key\s*===\s*['"]Escape['"]/, path + ' must close on Escape');
        assert.match(source, /focus\(/, path + ' must move focus into the dialog');
        assert.match(source, /aria-busy=/, path + ' must expose submission progress');
        assert.match(source, /role="alert"/, path + ' must retain visible submission errors');
    }
});

test('UI-1G keeps blocking surfaces opaque and free of animated or blurred backdrops', () => {
    for (const [path, source] of Object.entries(ui_1g_clean_sources)) {
        assert.doesNotMatch(source, /\bbackdrop-blur(?:-[\w\[\]/.-]+)?\b/, path + ' uses blur');
        assert.doesNotMatch(source, /\banimate-[\w\[\]-]+\b/, path + ' uses a persistent modal animation');
        assert.doesNotMatch(source, /\bshadow-(?:lg|xl|2xl)\b/, path + ' uses a large shadow');
        assert.doesNotMatch(source, /\btransition-all\b/, path + ' uses an unbounded transition');
        assert.doesNotMatch(source, /\bduration-(?:200|300|500|700|1000)\b/, path + ' exceeds the 150ms UI motion budget');
    }
});

test('UI-1G handles browser settings reads that resolve to undefined', () => {
    const settings = ui_1g_clean_sources['src/components/layout/SettingsModal.tsx'];
    const browser_settings = undefined;

    assert.equal(browser_settings ?? null, null, 'browser read IPC has no settings payload');
    assert.ok(
        /setSettings\(nextSettings \?\? null\)/.test(settings)
            || /if \(nextSettings\)\s*setSettings\(nextSettings\)/.test(settings),
        'the settings state must not receive undefined from the browser read mock',
    );
});

test('UI-1G keeps PhononImportModal open and surfaces submit failures', () => {
    const phonon_panel = ui_1f_clean_sources['src/components/panels/PhononPanel.tsx'];
    const submit_handler = phonon_panel.slice(
        phonon_panel.indexOf('const handleSubmitPhonon'),
        phonon_panel.indexOf('const handle_select_mode'),
    );
    const success_branch = submit_handler.indexOf('if (modesData)');

    assert.ok(success_branch >= 0, 'successful phonon import needs an explicit result branch');
    assert.doesNotMatch(
        submit_handler.slice(0, success_branch),
        /setIsPhononModalOpen\(false\)/,
        'the modal must remain mounted while the import IPC is pending',
    );
    assert.match(
        submit_handler,
        /if \(modesData\) \{[\s\S]*?setPhononModes\(modesData\)[\s\S]*?setIsPhononModalOpen\(false\)/,
        'the modal may close only after a successful import result',
    );
    assert.match(
        submit_handler,
        /catch \(cause\) \{[\s\S]*?setPanelError\(cause,[\s\S]*?throw cause;/,
        'the modal needs the rejected submission to render its own visible error state',
    );
});

test('UI-1G traps Tab focus inside every blocking dialog', () => {
    for (const [path, source] of Object.entries(ui_1g_clean_sources)) {
        assert.match(source, /event\.key\s*===\s*['"]Tab['"]/, path + ' must handle Tab focus traversal');
        assert.match(source, /querySelectorAll(?:<HTMLElement>)?\(/, path + ' must enumerate focusable descendants');
        assert.match(source, /event\.preventDefault\(\)/, path + ' must prevent focus from escaping the modal');
    }
});

test('UI-1G leaves Settings cancelable while the initial read is pending', () => {
    const settings = ui_1g_clean_sources['src/components/layout/SettingsModal.tsx'];

    assert.match(settings, /const isSaving = activeOperation !== null;/,
        'read progress and a settings mutation need distinct ownership');
    assert.match(settings, /busyRef\.current = isSaving;/,
        'Escape must remain available during a hanging read IPC');
    assert.doesNotMatch(settings, /onClick=\{onClose\}\s*disabled=\{isBusy\}/,
        'Cancel and close controls must not trap the user during initial loading');
    assert.match(settings, /onClick=\{onClose\}\s*disabled=\{isSaving\}/,
        'only an in-flight settings mutation may block cancellation');
});

test('UI-1G PromptModal callers return and propagate mutation promises', () => {
    const app = ui_1b_clean_sources['src/App.tsx'];
    const atom_operations = ui_1e_clean_sources['src/components/panels/AtomOperationsPanel.tsx'];
    const replacement_handler = atom_operations.slice(
        atom_operations.indexOf('const handleReplacementSubmit'),
        atom_operations.indexOf('const handleReplaceAtom'),
    );

    assert.match(
        app,
        /onSubmit:\s*async\s*\(elem\)\s*=>\s*\{[\s\S]*?await safeInvoke\('add_atom',/,
        'Add Atom must return the mutation promise instead of closing PromptModal immediately',
    );
    assert.doesNotMatch(
        app,
        /safeInvoke\('add_atom',[\s\S]*?\.catch\(e\s*=>\s*alert\(e\)\)/,
        'Add Atom must not swallow the rejection before PromptModal can render it',
    );
    assert.match(
        replacement_handler,
        /catch \(cause\) \{[\s\S]*?setMutationError\(cause,[\s\S]*?throw cause;/,
        'Replace Atom must propagate its rejected mutation after recording the panel error',
    );
});

const ui_3_source_paths = [
    'src/App.tsx',
    'src/components/layout/LeftSidebar.tsx',
    'src/components/layout/RightSidebar.tsx',
    'src/components/layout/TopNavBar.tsx',
    'src/components/layout/BottomStatusBar.tsx',
    'src/components/layout/Shell.tsx',
    'src/components/panels/index.ts',
    'src/hooks/useCameraInteraction.ts',
];

const ui_3_sources = Object.fromEntries(await Promise.all(ui_3_source_paths.map(async (path) => [
    path,
    await readFile(new URL('../' + path, import.meta.url), 'utf8'),
])));

const ui_3_clean_sources = Object.fromEntries(Object.entries(ui_3_sources).map(([path, source]) => [
    path,
    strip_comments(source),
]));

const ui_3_desktop_cases = Object.freeze([
    { viewport: '1280x720', theme: 'light' },
    { viewport: '1280x720', theme: 'dark' },
    { viewport: '1440x900', theme: 'light' },
    { viewport: '1440x900', theme: 'dark' },
    { viewport: '1920x1080', theme: 'light' },
    { viewport: '1920x1080', theme: 'dark' },
]);

function class_attribute(source, marker) {
    const start = source.indexOf(marker);
    assert.ok(start >= 0, 'missing marker: ' + marker);
    const tag_start = source.lastIndexOf('<', start);
    const tag_end = source.indexOf('>', start);
    assert.ok(tag_start >= 0 && tag_end >= 0, 'marker must occur in a JSX tag: ' + marker);
    return source.slice(tag_start, tag_end + 1);
}

function assert_ui_3_embedded_command_groups(top_nav) {
    for (const group of ['interaction', 'view', 'application']) {
        const tag = class_attribute(top_nav, `data-command-group="${group}"`);
        assert.doesNotMatch(tag, /\bcc-field\b|\bborder(?:-[\w\[\]/.]+)?\b|\bshadow(?:-[\w\[\]/.]+)?\b/,
            group + ' commands must stay embedded in the shared chrome, not a permanent boxed surface');
    }
    assert.match(class_attribute(top_nav, 'data-command-group="interaction"'), /\bshrink-0\b/,
        'interaction commands need a bounded desktop group');
    assert.match(class_attribute(top_nav, 'data-command-group="view"'), /\bshrink-0\b/,
        'view commands need a bounded desktop group');
    assert.match(class_attribute(top_nav, 'data-command-group="application"'), /\bmin-w-0\b/,
        'application commands must yield width before colliding with the centered view group');

    const tool_button = top_nav.slice(top_nav.indexOf('const ToolButton'), top_nav.indexOf('const ViewButton'));
    const view_button = top_nav.slice(top_nav.indexOf('const ViewButton'), top_nav.indexOf('const NavButton'));
    assert.match(tool_button, /\bhover:bg-/, 'embedded interaction buttons need a lightweight hover state');
    assert.match(tool_button, /\bbg-(?:white|emerald|slate)/, 'embedded interaction buttons need a lightweight active state');
    assert.doesNotMatch(tool_button, /\bshadow(?:-[\w\[\]/.]+)?\b/,
        'interaction buttons must not use a raised-card shadow for their active state');
    assert.match(view_button, /\bhover:bg-/, 'embedded view buttons need a lightweight hover state');
    assert.doesNotMatch(view_button, /(?:^|\s)(?:bg|border|shadow)(?:-[\w\[\]/.]+)?\b/,
        'view buttons must not have a permanent background, border, or shadow');
}

function assert_ui_3_status_hierarchy(bottom_status) {
    for (const group of ['summary', 'counters']) {
        const marker = `data-status-group="${group}"`;
        const tag = class_attribute(bottom_status, marker);
        const group_window = bottom_status.slice(bottom_status.indexOf(marker), bottom_status.indexOf(marker) + 640);

        assert.match(tag, /\bmin-w-0\b/, group + ' must permit bounded layout');
        assert.match(tag, /\boverflow-hidden\b/, group + ' must confine its own scroll viewport');
        assert.match(group_window,
            /<div className="(?=[^"]*\bw-full\b)(?=[^"]*\boverflow-x-auto\b)(?=[^"]*\boverflow-y-hidden\b)(?=[^"]*\bcustom-scrollbar\b)[^"]*">/,
            group + ' needs an intentional horizontal scroll path instead of silent clipping');
    }

    const counters = class_attribute(bottom_status, 'data-status-group="counters"');
    assert.match(counters, /\bw-\[[^\]]+\]/,
        'counters need an explicit width budget instead of consuming the summary track');
    assert.match(counters, /\bmax-w-\[[^\]]+\]/,
        'counters need a finite maximum width at large desktop sizes');
}

test('UI-3 documents every required desktop viewport and theme case', () => {
    assert.deepEqual(ui_3_desktop_cases, [
        { viewport: '1280x720', theme: 'light' },
        { viewport: '1280x720', theme: 'dark' },
        { viewport: '1440x900', theme: 'light' },
        { viewport: '1440x900', theme: 'dark' },
        { viewport: '1920x1080', theme: 'light' },
        { viewport: '1920x1080', theme: 'dark' },
    ]);
});

test('UI-3 omits the structure sidebar until a committed structure exists', () => {
    const left_sidebar = ui_3_clean_sources['src/components/layout/LeftSidebar.tsx'];
    const empty_branch = left_sidebar.slice(
        left_sidebar.indexOf('if (!crystalState || numAtoms === 0)'),
        left_sidebar.indexOf('const volume ='),
    );

    assert.match(empty_branch, /return null;/,
        'the unloaded workbench must return no structure sidebar at all');
    assert.doesNotMatch(empty_branch, /<aside\b|data-sidebar-surface="structure-workspace"|No structure loaded/,
        'the unloaded workbench must not reserve or paint a structure surface');
});

test('UI-3 gives the loaded sidebar responsive density with a 1920 table fit and a narrow scroll path', () => {
    const left_sidebar = ui_3_clean_sources['src/components/layout/LeftSidebar.tsx'];
    const loaded_sidebar = left_sidebar.slice(
        left_sidebar.indexOf('return (', left_sidebar.indexOf('const volume =')),
        left_sidebar.indexOf('const InfoRow'),
    );
    const loaded_aside = loaded_sidebar.match(/<aside\b[^>]*>/)?.[0];
    const atom_section = loaded_sidebar.slice(
        loaded_sidebar.indexOf('data-sidebar-section="atoms"'),
        loaded_sidebar.indexOf('</section>', loaded_sidebar.indexOf('data-sidebar-section="atoms"')),
    );
    const atom_table = atom_section.slice(atom_section.indexOf('<table'), atom_section.indexOf('</table>'));
    const sidebar_width = loaded_aside?.match(/(?:^|\s)w-\[(\d+)px\]/)?.[1];
    const table_width = atom_table.match(/\bmin-w-\[(\d+)px\]/)?.[1];

    assert.ok(loaded_aside, 'loaded structure content needs one semantic sidebar surface');
    assert.ok(Number(sidebar_width) > 0 && Number(sidebar_width) <= 300,
        'the default sidebar must stay narrow so the viewport remains dominant');
    assert.match(loaded_aside, /\b2xl:w-\[(?:3[2-9]\d|[4-9]\d{2,})px\]/,
        'the 1920 desktop sidebar must expand enough to fit all declared table columns');
    assert.doesNotMatch(loaded_aside, /\bw-\[340px\]/,
        'the sidebar must not remain fixed at its oversized 340px width');
    assert.ok(Number(table_width) >= 320,
        'the atom table needs an explicit 1920 desktop minimum instead of compressing seven columns');
    assert.match(atom_section, /\bmax-w-full\b/, 'the atom table scroll region must remain bounded by its sidebar');
    assert.match(atom_section, /\boverflow-x-auto\b/, 'narrow desktop windows need an intentional horizontal scroll path');
    for (const column of ['ID', 'El', 'x', 'y', 'z', 'Occ\\.', 'Color']) {
        assert.match(atom_table, new RegExp(`>${column}<`), 'missing declared critical atom column: ' + column);
    }
});

test('UI-3 gives the scientific tool rail an opaque separated surface without danger-state selection', () => {
    const right_sidebar = ui_3_clean_sources['src/components/layout/RightSidebar.tsx'];
    const rail = class_attribute(right_sidebar, 'data-tool-rail="scientific-tools"');
    const selected_branch = right_sidebar.slice(
        right_sidebar.indexOf("openAccordion === section.key"),
        right_sidebar.indexOf(')}', right_sidebar.indexOf("openAccordion === section.key")),
    );

    assert.match(rail, /\bcc-(?:chrome|panel)\b/, 'tool rail needs an opaque shared workbench surface');
    assert.match(rail, /\bborder-l\b/, 'tool rail needs a visible separator from the viewport');
    assert.match(rail, /\bmin-h-0\b/);
    assert.match(rail, /\boverflow-y-auto\b/);
    assert.doesNotMatch(rail, /\brounded(?:-[\w\[\]/.]+)?\b|\bshadow(?:-[\w\[\]/.]+)?\b/,
        'the tool rail itself must stay one continuous chrome surface');
    assert.match(right_sidebar, /aria-label=\{section\.label\}/);
    assert.match(right_sidebar, /aria-pressed=\{openAccordion === section\.key\}/);
    assert.doesNotMatch(selected_branch, /\b(?:bg|text|border|ring)-(?:red|rose)-/,
        'ordinary selected scientific tools must not use danger red as their only state signal');
    assert.doesNotMatch(selected_branch, /\b(?:shadow|ring)(?:-[\w\[\]/.]+)?\b/,
        'selected tool buttons must use an embedded state, not a raised or outlined card');
});

test('UI-3 preserves explicit brand, interaction, view, and application command groups', () => {
    const top_nav = ui_3_clean_sources['src/components/layout/TopNavBar.tsx'];

    for (const group of ['brand-global', 'interaction', 'view', 'application']) {
        assert.match(top_nav, new RegExp(`data-command-group="${group}"`),
            'top navigation needs an explicit ' + group + ' command group');
    }
    assert.match(top_nav, /<img[\s\S]*?alt="Logo"/, 'brand group must retain the accessible logo');
    assert.match(top_nav, /tooltip="Select"[\s\S]*?tooltip="Move"[\s\S]*?tooltip="Rotate"[\s\S]*?tooltip="Measure\/Select"/,
        'interaction group must retain all four interaction modes');
    assert.equal((top_nav.match(/safeInvoke\('set_camera_view_axis'/g) || []).length, 7,
        'view command ownership must remain in the top navigation');
    assert_ui_3_embedded_command_groups(top_nav);
});

test('UI-3 bounds both status groups without changing their scientific text ownership', () => {
    const bottom_status = ui_3_clean_sources['src/components/layout/BottomStatusBar.tsx'];

    assert_ui_3_status_hierarchy(bottom_status);
    for (const label of ['SpaceGroup:', 'Volume:', 'Phonon Mode', 'Bonds:', 'Total Atoms:', 'Selected:']) {
        assert.match(bottom_status, new RegExp(label.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')),
            'status text must remain visible in its owning group: ' + label);
    }
});

test('UI-3 rejects boxed command groups and status clipping regressions', () => {
    const bottom_status = ui_3_clean_sources['src/components/layout/BottomStatusBar.tsx'];
    const embedded_top = [
        '<div data-command-group="interaction" className="flex shrink-0 items-center">',
        '<div data-command-group="view" className="flex shrink-0 items-center">',
        '<div data-command-group="application" className="flex min-w-0 items-center">',
        'const ToolButton = () => <button className="hover:bg-slate-100 bg-emerald-100" />;',
        'const ViewButton = () => <button className="hover:bg-slate-100 hover:text-emerald-600" />;',
        'const NavButton = () => null;',
    ].join('\n');

    assert.throws(
        () => assert_ui_3_embedded_command_groups(embedded_top.replace(
            'className="flex shrink-0 items-center"',
            'className="cc-field flex shrink-0 items-center border shadow-sm"',
        )),
        /permanent boxed surface/,
        'semantic data markers must not permit a permanent command-group card',
    );
    assert.throws(
        () => assert_ui_3_embedded_command_groups(embedded_top.replace(
            'className="hover:bg-slate-100 hover:text-emerald-600"',
            'className="bg-slate-100 border shadow-sm hover:bg-slate-200"',
        )),
        /permanent background, border, or shadow/,
        'view buttons must not regress into individually boxed controls',
    );
    assert.throws(
        () => assert_ui_3_status_hierarchy(bottom_status.replace('overflow-x-auto', 'overflow-x-clip')),
        /intentional horizontal scroll path/,
        'bounded status text must not degrade into clipping',
    );
    assert.throws(
        () => assert_ui_3_status_hierarchy(bottom_status.replace(/\bw-\[[^\]]+\]\s+max-w-\[[^\]]+\]\s*/, '')),
        /explicit width budget/,
        'counters must not grow without an explicit desktop width budget',
    );
});

test('UI-3 freezes command, lazy, selection, listener, and viewport ownership during presentation repair', () => {
    const app = ui_3_clean_sources['src/App.tsx'];
    const left_sidebar = ui_3_clean_sources['src/components/layout/LeftSidebar.tsx'];
    const right_sidebar = ui_3_clean_sources['src/components/layout/RightSidebar.tsx'];
    const top_nav = ui_3_clean_sources['src/components/layout/TopNavBar.tsx'];
    const panel_index = ui_3_clean_sources['src/components/panels/index.ts'];
    const camera_interaction = ui_3_clean_sources['src/hooks/useCameraInteraction.ts'];
    const shell = ui_3_clean_sources['src/components/layout/Shell.tsx'];

    assert.equal((left_sidebar.match(/safeInvoke\('update_lattice_params'/g) || []).length, 1);
    assert.equal((top_nav.match(/safeInvoke\('set_camera_view_axis'/g) || []).length, 7);
    assert.equal((right_sidebar.match(/safeInvoke\(/g) || []).length, 0,
        'the presentation rail must not acquire scientific command ownership');
    assert.equal((right_sidebar.match(/safeListen\(/g) || []).length, 0,
        'the presentation rail must not acquire listener ownership');
    for (const panel of [
        'BondAnalysisPanel', 'VolumetricPanel', 'PhononPanel', 'BrillouinZonePanel', 'WannierPanel',
        'SupercellPanel', 'SlabPanel', 'AtomOperationsPanel', 'MeasurementPanel',
    ]) {
        assert.match(panel_index, new RegExp(`${panel}: \\(\\) => import\\('./${panel}'\\)`));
        assert.match(right_sidebar, new RegExp(`lazy\\(lazyConfig\\.${panel}\\)`));
        assert.doesNotMatch(right_sidebar, new RegExp(`from ['"]\\.\\.\\/panels\\/${panel}['"]`));
    }
    assert.match(app, /const \[selectedAtoms, setSelectedAtoms\] = useState<number\[\]>\(\[\]\)/);
    assert.doesNotMatch(left_sidebar, /useState<\s*CrystalState/);
    assert.equal((app.match(/safeInvoke\('get_crystal_state'/g) || []).length, 1);
    assert.match(shell, /ref=\{viewportRef\}/);
    assert.match(camera_interaction, /const el = viewportRef\.current;/);
    assert.doesNotMatch(right_sidebar, /\bonPointer(?:Down|Move|Up|Cancel)\s*=/);
});

test('UI-3 presentation surfaces retain the existing low-idle visual constraints', () => {
    for (const path of [
        'src/components/layout/LeftSidebar.tsx',
        'src/components/layout/RightSidebar.tsx',
        'src/components/layout/TopNavBar.tsx',
        'src/components/layout/BottomStatusBar.tsx',
        'src/components/layout/Shell.tsx',
    ]) {
        const source = ui_3_clean_sources[path];
        assert.doesNotMatch(source, /\bbackdrop-blur(?:-[\w\[\]/.-]+)?\b/, path + ' must not add blur');
        assert.doesNotMatch(source, /\b(?:bg-)?gradient(?:-to-[\w-]+)?\b/, path + ' must not add a gradient');
        assert.doesNotMatch(source, /\btransition-all\b/, path + ' must not add an unbounded transition');
    }
    assert.doesNotMatch(index_css, /@import\s+url\s*\(/i, 'UI-3 must not add a remote font import');
});
