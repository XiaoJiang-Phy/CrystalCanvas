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
const IPC_CONTRACT = resolve(FRONTEND_ROOT, 'ipc/contracts.ts');
const CRYSTAL_TYPES = resolve(FRONTEND_ROOT, 'types/crystal.ts');

const LEGACY_TYPE_ALIASES = {
    AppSettings: 'AppSettingsDto',
    BondAnalysisResult: 'BondAnalysisResult',
    BondInfo: 'BondInfo',
    BondLengthStat: 'BondLengthStat',
    BzInfoResponse: 'BzInfo',
    BzLabelPos: 'ScreenLabel',
    CoordinationInfo: 'CoordinationInfo',
    CrystalState: 'CrystalState',
    KPathInfoResponse: 'KPathInfo',
    KPathPointUi: 'KPathPointUi',
    KPathTextResponse: 'KPathText',
    MeasurementLabelPos: 'ScreenLabel',
    MeasurementOverlay: 'MeasurementOverlay',
    PhononModeSummary: 'PhononModeSummary',
    VolumetricInfo: 'VolumetricInfo',
    WannierInfo: 'WannierInfo',
};

const RUST_NAMED_TYPE_MAP = {
    ...LEGACY_TYPE_ALIASES,
};

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

function normalize_rust_type(type) {
    return type.replace(/\s+/g, ' ').trim();
}

function generic_parts(type) {
    const opening = type.indexOf('<');
    if (opening < 0 || !type.endsWith('>')) return null;
    return {
        name: type.slice(0, opening).split('::').at(-1),
        args: split_top_level_parameters(type.slice(opening + 1, -1)),
    };
}

function rust_type_schema(raw_type) {
    const type = normalize_rust_type(raw_type).replace(/^&(?:'[^ ]+\s+)?/, '');
    if (type === '()') return 'null';
    if (['String', 'str'].includes(type)) return 'string';
    if (type === 'bool') return 'boolean';
    if (/^(?:[iu](?:8|16|32|64|128|size)|f(?:32|64))$/.test(type)) return 'number';
    const array = type.match(/^\[([\s\S]+);\s*(\d+)\]$/);
    if (array) {
        const item = rust_type_schema(array[1]);
        return `[${Array.from({ length: Number(array[2]) }, () => item).join(',')}]`;
    }
    const generic = generic_parts(type);
    if (generic?.name === 'Option' && generic.args.length === 1) {
        return [rust_type_schema(generic.args[0]), 'null'].sort().join('|');
    }
    if (generic?.name === 'Vec' && generic.args.length === 1) {
        return `Array<${rust_type_schema(generic.args[0])}>`;
    }
    if (generic?.name === 'HashMap' && generic.args.length === 2) {
        return `Record<${rust_type_schema(generic.args[0])},${rust_type_schema(generic.args[1])}>`;
    }
    if (generic?.name === 'IpcEnumInput' && generic.args.length === 1) {
        return rust_type_schema(generic.args[0]);
    }
    if (generic && ['Result', 'IpcResult'].includes(generic.name)) {
        return rust_type_schema(generic.args[0]);
    }
    const name = type.split('::').at(-1);
    return RUST_NAMED_TYPE_MAP[name] ?? name;
}

function command_return_type(source, closing) {
    const tail = source.slice(closing + 1);
    const body = tail.indexOf('{');
    if (body < 0) return null;
    const signature_tail = tail.slice(0, body).trim();
    if (!signature_tail.startsWith('->')) return '()';
    return signature_tail.slice(2).trim();
}

function collect_backend_command_schemas(rust_sources) {
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
                return [{
                    name: snake_to_camel(name),
                    optional: generic_parts(normalize_rust_type(rust_type))?.name === 'Option',
                    rust_type: normalize_rust_type(rust_type),
                    type: rust_type_schema(rust_type),
                }];
            }).sort((left, right) => left.name.localeCompare(right.name));
            const return_type = command_return_type(source, closing);
            if (return_type === null) continue;
            commands[match[1]] = {
                args,
                result: rust_type_schema(return_type),
                rust_result: normalize_rust_type(return_type),
            };
        }
    }
    return sorted_record(commands);
}

function canonical_ts_type(node, source_file) {
    if (node.kind === ts.SyntaxKind.StringKeyword) return 'string';
    if (node.kind === ts.SyntaxKind.NumberKeyword) return 'number';
    if (node.kind === ts.SyntaxKind.BooleanKeyword) return 'boolean';
    if (node.kind === ts.SyntaxKind.NullKeyword) return 'null';
    if (node.kind === ts.SyntaxKind.UndefinedKeyword) return 'undefined';
    if (ts.isLiteralTypeNode(node)) {
        return ts.isStringLiteral(node.literal)
            ? JSON.stringify(node.literal.text)
            : node.getText(source_file).replace(/\s+/g, '');
    }
    if (ts.isParenthesizedTypeNode(node)) return canonical_ts_type(node.type, source_file);
    if (ts.isArrayTypeNode(node)) return `Array<${canonical_ts_type(node.elementType, source_file)}>`;
    if (ts.isTupleTypeNode(node)) {
        return `[${node.elements.map((element) => canonical_ts_type(element, source_file)).join(',')}]`;
    }
    if (ts.isUnionTypeNode(node)) {
        return node.types.map((type) => canonical_ts_type(type, source_file)).sort().join('|');
    }
    if (ts.isTypeReferenceNode(node)) {
        const name = node.typeName.getText(source_file);
        if (!node.typeArguments?.length) return name;
        return `${name}<${node.typeArguments.map((type) => canonical_ts_type(type, source_file)).join(',')}>`;
    }
    if (ts.isTypeLiteralNode(node)) return canonical_ts_members(node.members, source_file);
    return node.getText(source_file).replace(/\s+/g, '');
}

function canonical_ts_members(members, source_file) {
    return `{${members.flatMap((member) => {
        if (!ts.isPropertySignature(member) || !member.type || !member.name) return [];
        const name = member.name.getText(source_file).replace(/["']/g, '');
        return [`${name}${member.questionToken ? '?' : ''}:${canonical_ts_type(member.type, source_file)}`];
    }).sort().join(';')}}`;
}

function collect_ts_interfaces(source, path) {
    const source_file = ts.createSourceFile(path, source, ts.ScriptTarget.Latest, true);
    const interfaces = {};
    for (const statement of source_file.statements) {
        if (ts.isInterfaceDeclaration(statement)) {
            interfaces[statement.name.text] = {
                members: canonical_ts_members(statement.members, source_file),
                node: statement,
                source_file,
            };
        } else if (ts.isTypeAliasDeclaration(statement)) {
            interfaces[statement.name.text] = {
                members: canonical_ts_type(statement.type, source_file),
                node: statement,
                source_file,
            };
        }
    }
    return interfaces;
}

function collect_contract_command_schemas(contract_source) {
    const path = relative(ROOT, IPC_CONTRACT);
    const interfaces = collect_ts_interfaces(contract_source, path);
    const contract = interfaces.IpcCommandContract;
    if (!contract || !ts.isInterfaceDeclaration(contract.node)) return {};
    const commands = {};
    for (const member of contract.node.members) {
        if (!ts.isPropertySignature(member) || !member.type || !ts.isTypeLiteralNode(member.type)) continue;
        const name = member.name.getText(contract.source_file).replace(/["']/g, '');
        const args_member = member.type.members.find((item) => ts.isPropertySignature(item)
            && item.name.getText(contract.source_file).replace(/["']/g, '') === 'args');
        const result_member = member.type.members.find((item) => ts.isPropertySignature(item)
            && item.name.getText(contract.source_file).replace(/["']/g, '') === 'result');
        if (!args_member?.type || !result_member?.type) continue;
        const args = args_member.type.kind === ts.SyntaxKind.UndefinedKeyword
            ? []
            : ts.isTypeLiteralNode(args_member.type)
                ? args_member.type.members.flatMap((arg) => ts.isPropertySignature(arg) && arg.type
                    ? [{
                        name: arg.name.getText(contract.source_file).replace(/["']/g, ''),
                        optional: Boolean(arg.questionToken),
                        type: canonical_ts_type(arg.type, contract.source_file),
                    }]
                    : []).sort((left, right) => left.name.localeCompare(right.name))
                : null;
        commands[name] = { args, result: canonical_ts_type(result_member.type, contract.source_file) };
    }
    return commands;
}

function pascal_to_snake(name) {
    return name
        .replace(/([A-Z]+)([A-Z][a-z])/g, '$1_$2')
        .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
        .toLowerCase();
}

function serde_attribute_value(source, key) {
    return source.match(new RegExp(`serde\\s*\\([\\s\\S]*?${key}\\s*=\\s*"([^"]+)"[\\s\\S]*?\\)`))?.[1] ?? null;
}

function words(name) {
    return pascal_to_snake(name).split('_');
}

function apply_serde_case(name, style) {
    if (!style) return name;
    const parts = words(name);
    if (style === 'lowercase') return parts.join('');
    if (style === 'UPPERCASE') return parts.join('').toUpperCase();
    if (style === 'snake_case') return parts.join('_');
    if (style === 'SCREAMING_SNAKE_CASE') return parts.join('_').toUpperCase();
    if (style === 'kebab-case') return parts.join('-');
    if (style === 'SCREAMING-KEBAB-CASE') return parts.join('-').toUpperCase();
    if (style === 'camelCase') return parts[0] + parts.slice(1).map((part) => part[0].toUpperCase() + part.slice(1)).join('');
    if (style === 'PascalCase') return parts.map((part) => part[0].toUpperCase() + part.slice(1)).join('');
    return name;
}

function rust_named_dependencies(type) {
    return [...type.matchAll(/\b[A-Z][A-Za-z0-9_]*\b/g)].map((match) => match[0]);
}

function collect_rust_struct_schemas(rust_sources) {
    const structs = {};
    const pattern = /((?:#\s*\[[^\]]*\]\s*)*)pub\s+struct\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{/g;
    for (const source of rust_sources) {
        for (const match of source.matchAll(pattern)) {
            const opening = match.index + match[0].lastIndexOf('{');
            const closing = matching_delimiter(source, opening, '{', '}');
            if (closing < 0) continue;
            const rename_all = serde_attribute_value(match[1], 'rename_all');
            const body = source.slice(opening + 1, closing).replace(/\/\/.*$/gm, '');
            const dependencies = new Set();
            const fields = split_top_level_parameters(body).flatMap((raw_field) => {
                if (/serde\s*\([^)]*\b(?:skip|skip_serializing)\b/.test(raw_field)) return [];
                const field = strip_rust_attributes(raw_field);
                const field_match = field.match(/^pub\s+([A-Za-z_][A-Za-z0-9_]*)\s*:\s*([\s\S]+)$/);
                if (!field_match) return [];
                rust_named_dependencies(field_match[2]).forEach((name) => dependencies.add(name));
                const serialized_name = serde_attribute_value(raw_field, 'rename')
                    ?? apply_serde_case(field_match[1], rename_all);
                const optional = /serde\s*\([^)]*\bskip_serializing_if\b/.test(raw_field) ? '?' : '';
                return [`${serialized_name}${optional}:${rust_type_schema(field_match[2])}`];
            }).sort();
            structs[match[2]] = { schema: `{${fields.join(';')}}`, dependencies: [...dependencies] };
        }
    }
    return structs;
}

function collect_rust_enum_schemas(rust_sources) {
    const enums = {};
    const pattern = /((?:#\s*\[[^\]]*\]\s*)*)pub\s+enum\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{/g;
    for (const source of rust_sources) {
        for (const match of source.matchAll(pattern)) {
            const opening = match.index + match[0].lastIndexOf('{');
            const closing = matching_delimiter(source, opening, '{', '}');
            if (closing < 0) continue;
            const has_serde = /\b(?:serde::)?(?:Serialize|Deserialize)\b/.test(match[1]);
            if (Object.hasOwn(enums, match[2]) && !has_serde) continue;
            const rename_all = serde_attribute_value(match[1], 'rename_all');
            const variants = split_top_level_parameters(source.slice(opening + 1, closing)
                .replace(/\/\/.*$/gm, ''))
                .flatMap((raw_variant) => {
                    const variant = strip_rust_attributes(raw_variant).trim();
                    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(variant)) return [];
                    return [serde_attribute_value(raw_variant, 'rename') ?? apply_serde_case(variant, rename_all)];
                })
                .map((variant) => JSON.stringify(variant))
                .sort();
            enums[match[2]] = variants.join('|');
        }
    }
    return enums;
}

const SUPPORTED_RUST_GENERICS = new Set(['HashMap', 'IpcEnumInput', 'IpcResult', 'Option', 'Result', 'Vec']);
const RUST_SCHEMA_WRAPPERS = new Set(['HashMap', 'IpcEnumInput', 'IpcResult', 'Option', 'Result', 'Vec']);

function unsupported_generic_names(type) {
    const generic = generic_parts(normalize_rust_type(type));
    if (!generic) return [];
    return [
        ...(SUPPORTED_RUST_GENERICS.has(generic.name) ? [] : [generic.name]),
        ...generic.args.flatMap(unsupported_generic_names),
    ];
}

function collect_rust_declarations(rust_sources) {
    const declarations = {};
    const pattern = /((?:#\s*\[[^\]]*\]\s*)*)pub\s+(struct|enum)\s+([A-Za-z_][A-Za-z0-9_]*)/g;
    for (const source of rust_sources) {
        for (const match of source.matchAll(pattern)) {
            let cursor = match.index + match[0].length;
            while (/\s/.test(source[cursor])) cursor += 1;
            let generic = source[cursor] === '<';
            if (generic) {
                const closing = matching_delimiter(source, cursor, '<', '>');
                if (closing < 0) continue;
                cursor = closing + 1;
            }
            while (/\s/.test(source[cursor])) cursor += 1;
            const where_clause = source.slice(cursor, cursor + 5) === 'where';
            if (where_clause) {
                const opening = source.indexOf('{', cursor + 5);
                if (opening < 0) continue;
                cursor = opening;
            }
            const delimiter = source[cursor];
            let body = '';
            if (delimiter === '{') {
                const closing = matching_delimiter(source, cursor, '{', '}');
                if (closing < 0) continue;
                body = source.slice(cursor + 1, closing).replace(/\/\/.*$/gm, '');
            }
            const field_types = [];
            const dependencies = new Set();
            let private_serialized_field = false;
            let flatten = false;
            let payload_variant = false;
            if (match[2] === 'struct' && delimiter === '{') {
                const serializable = /\b(?:serde::)?Serialize\b/.test(match[1]);
                for (const raw_field of split_top_level_parameters(body)) {
                    flatten ||= /serde\s*\([^)]*\bflatten\b/.test(raw_field);
                    const skipped = /serde\s*\([^)]*\b(?:skip|skip_serializing)\b/.test(raw_field);
                    const field = strip_rust_attributes(raw_field);
                    const field_match = field.match(/^(pub(?:\([^)]*\))?\s+)?[A-Za-z_][A-Za-z0-9_]*\s*:\s*([\s\S]+)$/);
                    if (!field_match) continue;
                    if (serializable && !field_match[1] && !skipped) private_serialized_field = true;
                    field_types.push(field_match[2]);
                    rust_named_dependencies(field_match[2]).forEach((name) => dependencies.add(name));
                }
            } else if (match[2] === 'enum' && delimiter === '{') {
                for (const raw_variant of split_top_level_parameters(body)) {
                    const variant = strip_rust_attributes(raw_variant).trim();
                    if (/^[A-Za-z_][A-Za-z0-9_]*\s*[({]/.test(variant)) payload_variant = true;
                    rust_named_dependencies(variant).forEach((name) => dependencies.add(name));
                }
            }
            append(declarations, match[3], {
                kind: match[2],
                generic,
                tuple: match[2] === 'struct' && delimiter === '(',
                where_clause,
                private_serialized_field,
                flatten,
                tagged: match[2] === 'enum' && /serde\s*\([^)]*\b(?:untagged|tag|content)\b/.test(match[1]),
                payload_variant,
                field_types,
                dependencies: [...dependencies],
            });
        }
    }
    return declarations;
}

function collect_unsupported_rust_wire_shapes(rust_sources, command_schemas) {
    const declarations = collect_rust_declarations(rust_sources);
    const reachable = new Set();
    const queue = Object.values(command_schemas).flatMap((schema) => [
        schema.rust_result,
        ...schema.args.map((arg) => arg.rust_type),
    ]).flatMap(rust_named_dependencies);
    const issues = [];
    while (queue.length > 0) {
        const name = queue.pop();
        if (reachable.has(name) || RUST_SCHEMA_WRAPPERS.has(name)) continue;
        const matches = declarations[name];
        if (!matches) continue;
        reachable.add(name);
        if (matches.length !== 1) {
            issues.push({ type: name, reason: 'multiple reachable Rust declarations share this name' });
            continue;
        }
        const declaration = matches[0];
        if (declaration.generic) issues.push({ type: name, reason: 'generic wire declarations are unsupported' });
        if (declaration.tuple) issues.push({ type: name, reason: 'tuple struct wire declarations are unsupported' });
        if (declaration.where_clause) issues.push({ type: name, reason: 'wire declarations with where clauses are unsupported' });
        if (declaration.private_serialized_field) issues.push({ type: name, reason: 'private serialized fields are unsupported' });
        if (declaration.flatten) issues.push({ type: name, reason: 'serde(flatten) is unsupported' });
        if (declaration.tagged) issues.push({ type: name, reason: 'tagged or untagged serde enums are unsupported' });
        if (declaration.payload_variant) issues.push({ type: name, reason: 'enum payload variants are unsupported' });
        for (const field_type of declaration.field_types) {
            for (const generic_name of unsupported_generic_names(field_type)) {
                issues.push({ type: name, reason: `unsupported Rust container ${generic_name}` });
            }
        }
        queue.push(...declaration.dependencies);
    }
    return issues.sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right)));
}

function collect_reachable_wire_types(command_schemas, rust_structs, rust_enums) {
    const reachable = new Set();
    const queue = Object.values(command_schemas).flatMap((schema) => [
        schema.rust_result,
        ...schema.args.map((arg) => arg.rust_type),
    ]).flatMap(rust_named_dependencies);
    while (queue.length > 0) {
        const name = queue.pop();
        if (reachable.has(name) || (!rust_structs[name] && !Object.hasOwn(rust_enums, name))) continue;
        reachable.add(name);
        if (rust_structs[name]) queue.push(...rust_structs[name].dependencies);
    }
    return reachable;
}

function compare_command_schemas(backend, frontend) {
    return Object.entries(backend).flatMap(([command, rust_schema]) => {
        const ts_schema = frontend[command];
        if (!ts_schema) return [{ command, reason: 'missing TypeScript command contract' }];
        if (ts_schema.args === null) return [{ command, reason: 'TypeScript args must be an inline object or undefined' }];
        const failures = [];
        if (JSON.stringify(rust_schema.args.map(({ name, optional, type }) => ({ name, optional, type })))
            !== JSON.stringify(ts_schema.args)) failures.push({ command, reason: 'argument schema mismatch', rust: rust_schema.args, typescript: ts_schema.args });
        if (rust_schema.result !== ts_schema.result) failures.push({ command, reason: 'result schema mismatch', rust: rust_schema.result, typescript: ts_schema.result });
        return failures;
    });
}

function compare_dto_schemas(rust_structs, ts_interfaces, reachable) {
    return [...reachable].filter((name) => rust_structs[name]).flatMap((rust_name) => {
        const ts_name = LEGACY_TYPE_ALIASES[rust_name] ?? rust_name;
        const rust_schema = rust_structs[rust_name].schema;
        const ts_schema = ts_interfaces[ts_name]?.members;
        if (!ts_schema) return [{ rust: rust_name, typescript: ts_name, reason: 'missing TypeScript DTO' }];
        return rust_schema === ts_schema ? [] : [{ rust: rust_name, typescript: ts_name, rust_schema, typescript_schema: ts_schema }];
    });
}

function compare_enum_schemas(rust_enums, ts_interfaces, reachable) {
    return [...reachable].filter((name) => Object.hasOwn(rust_enums, name)).flatMap((name) => {
        const rust_schema = rust_enums[name];
        const ts_name = LEGACY_TYPE_ALIASES[name] ?? name;
        const ts_schema = ts_interfaces[ts_name]?.members;
        if (!ts_schema) return [{ rust: name, typescript: ts_name, reason: 'missing TypeScript IPC enum' }];
        return rust_schema === ts_schema ? [] : [{ rust: name, typescript: ts_name, rust_schema, typescript_schema: ts_schema }];
    });
}

function indexed_calls(calls, key) {
    const result = {};
    for (const call of calls) append(result, call[key], call.file);
    return sorted_record(result);
}

export async function create_inventory() {
    const [main_source, command_source, event_source, contract_source, crystal_types_source, frontend_files, rust_files] = await Promise.all([
        readFile(RUST_MAIN, 'utf8'),
        readFile(CLASSIFICATION, 'utf8'),
        readFile(EVENT_CLASSIFICATION, 'utf8'),
        readFile(IPC_CONTRACT, 'utf8'),
        readFile(CRYSTAL_TYPES, 'utf8'),
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
    const rust_source_texts = rust_sources.map(({ source }) => source);
    const backend_command_schemas = collect_backend_command_schemas(rust_source_texts);
    const backend_command_args = Object.fromEntries(Object.entries(backend_command_schemas)
        .map(([command, schema]) => [command, schema.args.map(({ name }) => name)]));
    const contract_command_schemas = collect_contract_command_schemas(contract_source);
    const rust_dto_schemas = collect_rust_struct_schemas(rust_source_texts);
    const rust_enum_schemas = collect_rust_enum_schemas(rust_source_texts);
    const reachable_wire_types = collect_reachable_wire_types(
        backend_command_schemas,
        rust_dto_schemas,
        rust_enum_schemas,
    );
    const unsupported_rust_wire_shapes = collect_unsupported_rust_wire_shapes(
        rust_source_texts,
        backend_command_schemas,
    );
    const ts_dto_schemas = {
        ...collect_ts_interfaces(crystal_types_source, relative(ROOT, CRYSTAL_TYPES)),
        ...collect_ts_interfaces(contract_source, relative(ROOT, IPC_CONTRACT)),
    };

    for (const { path, source } of rust_sources) {
        const file = relative(ROOT, path);
        for (const event of collect_backend_events(source)) append(backend_events, event, file);
    }

    const inventory = {
        backend_commands,
        backend_command_args,
        backend_command_schemas,
        command_schema_mismatches: compare_command_schemas(backend_command_schemas, contract_command_schemas),
        dto_schema_mismatches: compare_dto_schemas(rust_dto_schemas, ts_dto_schemas, reachable_wire_types),
        enum_schema_mismatches: compare_enum_schemas(rust_enum_schemas, ts_dto_schemas, reachable_wire_types),
        unsupported_rust_wire_shapes,
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
    command_schema_mismatches = [],
    dto_schema_mismatches = [],
    enum_schema_mismatches = [],
    unsupported_rust_wire_shapes = [],
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
        command_schema_mismatches,
        dto_schema_mismatches,
        enum_schema_mismatches,
        unsupported_rust_wire_shapes,
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
        ...inventory.command_schema_mismatches.map((mismatch) => `command schema mismatch for ${mismatch.command}: ${mismatch.reason}; Rust=${JSON.stringify(mismatch.rust)} TypeScript=${JSON.stringify(mismatch.typescript)}`),
        ...inventory.dto_schema_mismatches.map((mismatch) => `DTO schema mismatch for Rust ${mismatch.rust} / TypeScript ${mismatch.typescript}: Rust=${mismatch.rust_schema ?? mismatch.reason} TypeScript=${mismatch.typescript_schema ?? mismatch.reason}`),
        ...inventory.enum_schema_mismatches.map((mismatch) => `enum schema mismatch for Rust ${mismatch.rust} / TypeScript ${mismatch.typescript}: Rust=${mismatch.rust_schema ?? mismatch.reason} TypeScript=${mismatch.typescript_schema ?? mismatch.reason}`),
        ...inventory.unsupported_rust_wire_shapes.map(({ type, reason }) => `unsupported Rust wire shape ${type}: ${reason}`),
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
