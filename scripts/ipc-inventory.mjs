import { readFile, readdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { relative, resolve } from 'node:path';
import ts from 'typescript';

const ROOT = resolve(fileURLToPath(new URL('..', import.meta.url)));
const FRONTEND_ROOT = resolve(ROOT, 'src');
const RUST_ROOT = resolve(ROOT, 'src-tauri/src');
const RUST_MAIN = resolve(RUST_ROOT, 'main.rs');
const IPC_ADAPTER = resolve(FRONTEND_ROOT, 'utils/tauri-mock.ts');
const CLASSIFICATION = resolve(ROOT, 'ipc/command-classification.json');
const EVENT_CLASSIFICATION = resolve(ROOT, 'ipc/event-classification.json');
const INVENTORY = resolve(ROOT, 'ipc/inventory.json');

async function walk(directory) {
    const entries = await readdir(directory, { withFileTypes: true });
    const paths = await Promise.all(entries
        .sort((left, right) => left.name.localeCompare(right.name))
        .map(async (entry) => {
            const path = resolve(directory, entry.name);
            return entry.isDirectory() ? walk(path) : [path];
        }));
    return paths.flat();
}

function append(map, key, value) {
    (map[key] ??= []).push(value);
}

function sorted_record(record) {
    return Object.fromEntries(Object.entries(record)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, values]) => [key, Array.isArray(values) ? [...values].sort() : values]));
}

function property_names(node) {
    if (!node) return [];
    if (!ts.isObjectLiteralExpression(node)) return null;
    if (node.properties.some((property) => !ts.isPropertyAssignment(property)
        && !ts.isShorthandPropertyAssignment(property))) return null;
    const names = node.properties.flatMap((property) => {
        if (!ts.isPropertyAssignment(property) && !ts.isShorthandPropertyAssignment(property)) return [];
        return ts.isIdentifier(property.name) || ts.isStringLiteral(property.name)
            ? [property.name.text]
            : [];
    }).sort();
    return names.length === node.properties.length ? names : null;
}

function type_argument_text(source_file, call) {
    return call.typeArguments?.[0]?.getText(source_file) ?? null;
}

function scan_frontend_file(path, source) {
    const source_file = ts.createSourceFile(path, source, ts.ScriptTarget.Latest, true);
    const command_calls = [];
    const event_calls = [];
    const dynamic_commands = [];
    const dynamic_events = [];
    const invalid_command_args = [];
    const unsafe_type_arguments = [];
    const forbidden_tauri_imports = [];
    const forbidden_ipc_reexports = [];
    const invoke_names = new Set();
    const listen_names = new Set();
    const relative_path = relative(ROOT, path);

    for (const statement of source_file.statements) {
        if (ts.isExportDeclaration(statement) && statement.moduleSpecifier
            && ts.isStringLiteral(statement.moduleSpecifier)
            && statement.moduleSpecifier.text.includes('utils/tauri-mock')) {
            forbidden_ipc_reexports.push({ file: relative_path, module: statement.moduleSpecifier.text });
        }
        if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier)) continue;
        const module_name = statement.moduleSpecifier.text;
        if (module_name.includes('utils/tauri-mock') && statement.importClause?.namedBindings
            && ts.isNamedImports(statement.importClause.namedBindings)) {
            for (const item of statement.importClause.namedBindings.elements) {
                if (item.propertyName?.text === 'safeInvoke' || item.name.text === 'safeInvoke') invoke_names.add(item.name.text);
                if (item.propertyName?.text === 'safeListen' || item.name.text === 'safeListen') listen_names.add(item.name.text);
            }
        }
        if (module_name.startsWith('@tauri-apps/') && path !== IPC_ADAPTER) {
            forbidden_tauri_imports.push({ file: relative_path, module: module_name });
        }
    }

    for (const statement of source_file.statements) {
        if (ts.isExportDeclaration(statement) && !statement.moduleSpecifier && statement.exportClause
            && ts.isNamedExports(statement.exportClause)
            && statement.exportClause.elements.some((item) => invoke_names.has(item.propertyName?.text ?? item.name.text)
                || listen_names.has(item.propertyName?.text ?? item.name.text))) {
            forbidden_ipc_reexports.push({ file: relative_path, module: '<local IPC re-export>' });
        }
    }

    function visit(node) {
        if (ts.isCallExpression(node)) {
            if (node.expression.kind === ts.SyntaxKind.ImportKeyword
                && ts.isStringLiteral(node.arguments[0])
                && node.arguments[0].text.startsWith('@tauri-apps/')
                && path !== IPC_ADAPTER) {
                forbidden_tauri_imports.push({ file: relative_path, module: node.arguments[0].text });
            }
            if (ts.isIdentifier(node.expression) && invoke_names.has(node.expression.text)) {
                const command = node.arguments[0];
                if (ts.isStringLiteral(command)) {
                    const args = property_names(node.arguments[1]);
                    if (args === null) invalid_command_args.push({ file: relative_path, expression: node.arguments[1]?.getText(source_file) ?? '<missing>' });
                    if (node.typeArguments?.length) unsafe_type_arguments.push({ file: relative_path, expression: node.getText(source_file) });
                    command_calls.push({
                        command: command.text,
                        file: relative_path,
                        args: args ?? [],
                        response_type: type_argument_text(source_file, node),
                    });
                } else {
                    dynamic_commands.push({ file: relative_path, expression: command?.getText(source_file) ?? '<missing>' });
                }
            }
            if (ts.isIdentifier(node.expression) && listen_names.has(node.expression.text)) {
                const event = node.arguments[0];
                if (ts.isStringLiteral(event)) {
                    event_calls.push({
                        event: event.text,
                        file: relative_path,
                        payload_type: type_argument_text(source_file, node),
                    });
                } else {
                    dynamic_events.push({ file: relative_path, expression: event?.getText(source_file) ?? '<missing>' });
                }
            }
        }
        ts.forEachChild(node, visit);
    }

    visit(source_file);
    return { command_calls, event_calls, dynamic_commands, dynamic_events, invalid_command_args, unsafe_type_arguments, forbidden_tauri_imports, forbidden_ipc_reexports };
}

function collect_backend_events(source) {
    return [...source.matchAll(/\.emit\(\s*"([^"]+)"/g)].map((match) => match[1]);
}

function matching_delimiter(source, opening_index, opening, closing) {
    let depth = 0;
    for (let index = opening_index; index < source.length; index += 1) {
        if (source[index] === opening) depth += 1;
        if (source[index] === closing) depth -= 1;
        if (depth === 0) return index;
    }
    return -1;
}

function split_top_level_parameters(source) {
    const parameters = [];
    let start = 0;
    let angle_depth = 0;
    let parenthesis_depth = 0;
    let bracket_depth = 0;
    let brace_depth = 0;
    for (let index = 0; index < source.length; index += 1) {
        const character = source[index];
        if (character === '<') angle_depth += 1;
        else if (character === '>') angle_depth -= 1;
        else if (character === '(') parenthesis_depth += 1;
        else if (character === ')') parenthesis_depth -= 1;
        else if (character === '[') bracket_depth += 1;
        else if (character === ']') bracket_depth -= 1;
        else if (character === '{') brace_depth += 1;
        else if (character === '}') brace_depth -= 1;
        else if (character === ',' && angle_depth === 0 && parenthesis_depth === 0
            && bracket_depth === 0 && brace_depth === 0) {
            parameters.push(source.slice(start, index).trim());
            start = index + 1;
        }
    }
    const tail = source.slice(start).trim();
    if (tail) parameters.push(tail);
    return parameters;
}

function strip_rust_attributes(parameter) {
    let result = parameter.trim();
    while (result.startsWith('#[')) {
        const closing = matching_delimiter(result, 1, '[', ']');
        if (closing < 0) break;
        result = result.slice(closing + 1).trim();
    }
    return result;
}

function snake_to_camel(name) {
    return name.replace(/_([a-z0-9])/g, (_, character) => character.toUpperCase());
}

function collect_backend_command_args(rust_sources) {
    const commands = {};
    const command_pattern = /#\s*\[\s*tauri::command(?:\s*\([^)]*\))?\s*\]\s*pub\s+(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(/g;
    for (const source of rust_sources) {
        for (const match of source.matchAll(command_pattern)) {
            const opening = match.index + match[0].lastIndexOf('(');
            const closing = matching_delimiter(source, opening, '(', ')');
            if (closing < 0) continue;
            const args = split_top_level_parameters(source.slice(opening + 1, closing)).flatMap((raw_parameter) => {
                const parameter = strip_rust_attributes(raw_parameter);
                const separator = parameter.indexOf(':');
                if (separator < 0) return [];
                const name = parameter.slice(0, separator).trim().replace(/^mut\s+/, '');
                const rust_type = parameter.slice(separator + 1).trim();
                if (/\bState\s*</.test(rust_type)
                    || /\b(?:AppHandle|WebviewWindow|Window)\b/.test(rust_type)) return [];
                return [snake_to_camel(name)];
            }).sort();
            commands[match[1]] = args;
        }
    }
    return sorted_record(commands);
}

function indexed_calls(calls, key) {
    const result = {};
    for (const call of calls) append(result, call[key], call.file);
    return sorted_record(result);
}

export async function create_inventory() {
    const [main_source, command_source, event_source, frontend_files, rust_files] = await Promise.all([
        readFile(RUST_MAIN, 'utf8'),
        readFile(CLASSIFICATION, 'utf8'),
        readFile(EVENT_CLASSIFICATION, 'utf8'),
        walk(FRONTEND_ROOT),
        walk(RUST_ROOT),
    ]);
    const classification = JSON.parse(command_source).commands;
    const event_classification = JSON.parse(event_source).events;
    const backend_commands = [...main_source.matchAll(/commands::([a-zA-Z0-9_]+)(?=\s*(?:,|\]))/g)]
        .map((match) => match[1])
        .sort();
    const scans = await Promise.all(frontend_files
        .filter((path) => /\.(ts|tsx)$/.test(path))
        .map(async (path) => scan_frontend_file(path, await readFile(path, 'utf8'))));
    const frontend_command_calls = scans.flatMap((scan) => scan.command_calls)
        .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right)));
    const frontend_event_calls = scans.flatMap((scan) => scan.event_calls)
        .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right)));
    const backend_events = {};
    const rust_sources = await Promise.all(rust_files
        .filter((file) => file.endsWith('.rs'))
        .map(async (path) => ({ path, source: await readFile(path, 'utf8') })));
    const backend_command_args = collect_backend_command_args(rust_sources.map(({ source }) => source));

    for (const { path, source } of rust_sources) {
        const file = relative(ROOT, path);
        for (const event of collect_backend_events(source)) append(backend_events, event, file);
    }

    const inventory = {
        backend_commands,
        backend_command_args,
        missing_backend_command_argument_schemas: backend_commands
            .filter((command) => !Object.hasOwn(backend_command_args, command)),
        backend_events: sorted_record(backend_events),
        classification: sorted_record(classification),
        event_classification: sorted_record(event_classification),
        frontend_command_calls,
        frontend_commands: indexed_calls(frontend_command_calls, 'command'),
        frontend_event_calls,
        frontend_events: indexed_calls(frontend_event_calls, 'event'),
        forbidden_tauri_imports: scans.flatMap((scan) => scan.forbidden_tauri_imports).sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right))),
        dynamic_frontend_commands: scans.flatMap((scan) => scan.dynamic_commands).sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right))),
        dynamic_frontend_events: scans.flatMap((scan) => scan.dynamic_events).sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right))),
        invalid_frontend_command_args: scans.flatMap((scan) => scan.invalid_command_args).sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right))),
        unsafe_frontend_type_arguments: scans.flatMap((scan) => scan.unsafe_type_arguments).sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right))),
        forbidden_ipc_reexports: scans.flatMap((scan) => scan.forbidden_ipc_reexports).sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right))),
    };
    return { ...inventory, ...analyze_contract(inventory) };
}

export function analyze_contract({
    backend_commands,
    backend_command_args = {},
    frontend_command_calls = [],
    frontend_commands = {},
    classification,
    backend_events = {},
    frontend_events = {},
    event_classification = {},
    forbidden_tauri_imports = [],
    dynamic_frontend_commands = [],
    dynamic_frontend_events = [],
    invalid_frontend_command_args = [],
    unsafe_frontend_type_arguments = [],
    forbidden_ipc_reexports = [],
    missing_backend_command_argument_schemas = [],
}) {
    const registered = new Set(backend_commands);
    const frontend = Object.keys(frontend_commands).sort();
    const classified = Object.keys(classification).sort();
    const classified_events = new Set(Object.keys(event_classification));
    const argument_mismatches = frontend_command_calls.flatMap((call) => {
        const expected = backend_command_args[call.command];
        if (!expected || JSON.stringify(call.args) === JSON.stringify(expected)) return [];
        return [{ command: call.command, file: call.file, expected, actual: call.args }];
    });
    return {
        argument_mismatches,
        missing_backend_command_argument_schemas,
        dynamic_frontend_commands,
        dynamic_frontend_events,
        invalid_frontend_command_args,
        unsafe_frontend_type_arguments,
        forbidden_ipc_reexports,
        forbidden_tauri_imports,
        unclassified_backend_commands: backend_commands.filter((command) => !Object.hasOwn(classification, command)),
        unclassified_backend_events: Object.keys(backend_events).sort().filter((event) => !classified_events.has(event)),
        unclassified_frontend_commands: frontend.filter((command) => !Object.hasOwn(classification, command)),
        unclassified_frontend_events: Object.keys(frontend_events).sort().filter((event) => !classified_events.has(event)),
        unregistered_classified_commands: classified.filter((command) => !registered.has(command)),
        unregistered_frontend_commands: frontend.filter((command) => !registered.has(command)),
    };
}

export async function write_inventory() {
    await writeFile(INVENTORY, `${JSON.stringify(await create_inventory(), null, 2)}\n`);
}

export function assert_contract(inventory) {
    const failures = [
        ...inventory.argument_mismatches.map(({ command, file, expected, actual }) => `argument mismatch for ${command} in ${file}: expected [${expected.join(', ')}], got [${actual.join(', ')}]`),
        ...inventory.missing_backend_command_argument_schemas.map((command) => `missing backend argument schema: ${command}`),
        ...inventory.dynamic_frontend_commands.map(({ file, expression }) => `dynamic frontend command in ${file}: ${expression}`),
        ...inventory.dynamic_frontend_events.map(({ file, expression }) => `dynamic frontend event in ${file}: ${expression}`),
        ...inventory.invalid_frontend_command_args.map(({ file, expression }) => `non-literal frontend command args in ${file}: ${expression}`),
        ...inventory.unsafe_frontend_type_arguments.map(({ file, expression }) => `forbidden IPC type argument in ${file}: ${expression}`),
        ...inventory.forbidden_ipc_reexports.map(({ file, module }) => `forbidden IPC re-export in ${file}: ${module}`),
        ...inventory.forbidden_tauri_imports.map(({ file, module }) => `forbidden Tauri import in ${file}: ${module}`),
        ...inventory.unclassified_backend_commands.map((command) => `unclassified backend command: ${command}`),
        ...inventory.unclassified_backend_events.map((event) => `unclassified backend event: ${event}`),
        ...inventory.unclassified_frontend_commands.map((command) => `unclassified frontend command: ${command}`),
        ...inventory.unclassified_frontend_events.map((event) => `unclassified frontend event: ${event}`),
        ...inventory.unregistered_classified_commands.map((command) => `classified command is not registered: ${command}`),
        ...inventory.unregistered_frontend_commands.map((command) => `unregistered frontend command: ${command}`),
    ];
    if (failures.length > 0) throw new Error(failures.join('\n'));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await write_inventory();
