#!/usr/bin/env node
/**
 * Production reachability census for Focus Orb.
 *
 * Precision choices are deliberate:
 * - TypeScript is parsed with the installed compiler API, including resolved Promise return types.
 * - Python import/reference work is delegated to `reachability-python.py` and stdlib `ast`.
 * - Rust dead code is checked by Clippy; source scanning only enforces reasons on suppressions.
 * - An exported callable becomes a finding only when tests reach it and production does not.  This
 *   catches "built + unit-tested + unreachable" without pretending every unused public API is a
 *   product bug.  Completely unreferenced public APIs remain in the published scan denominator.
 */
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, extname, isAbsolute, join, relative, resolve, sep } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import tsImport from 'typescript';

const ts = tsImport.default ?? tsImport;

function parseArgs(argv) {
  const result = { root: process.cwd(), config: undefined, json: false, skipClippy: false };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--root') result.root = argv[++index];
    else if (arg === '--config') result.config = argv[++index];
    else if (arg === '--json') result.json = true;
    else if (arg === '--skip-clippy') result.skipClippy = true;
    else throw new Error(`unknown argument: ${arg}`);
  }
  result.root = resolve(result.root);
  result.config = resolve(result.config ?? join(result.root, 'tooling/reachability.config.json'));
  return result;
}

function repoRelative(root, path) {
  return relative(root, resolve(path)).split(sep).join('/');
}

function isInside(root, path) {
  const candidate = relative(resolve(root), resolve(path));
  return candidate === '' || (!candidate.startsWith(`..${sep}`) && candidate !== '..' && !isAbsolute(candidate));
}

function isTestPath(path) {
  const normalized = path.split(sep).join('/');
  return /(^|\/)(tests?|__tests__|fixtures?)(\/|$)|(?:^|[._-])(test|spec)\.[^.]+$/i.test(normalized);
}

function walk(root, configuredRoots, extensions) {
  const files = [];
  const visit = (path) => {
    let status;
    try {
      status = statSync(path);
    } catch {
      return;
    }
    if (status.isDirectory()) {
      const name = path.split(sep).at(-1);
      if (['node_modules', '.venv', 'target', 'build', 'dist', '.git', '.gate-tools', '.gate-logs'].includes(name)) return;
      for (const entry of readdirSync(path)) visit(join(path, entry));
      return;
    }
    if (extensions.has(extname(path))) files.push(resolve(path));
  };
  for (const configured of configuredRoots) visit(join(root, configured));
  return [...new Set(files)].sort();
}

function loadPrograms(root, config) {
  return config.typescript.tsconfigs.map((configuredPath) => {
    const configPath = join(root, configuredPath);
    const loaded = ts.readConfigFile(configPath, ts.sys.readFile);
    if (loaded.error) {
      throw new Error(ts.flattenDiagnosticMessageText(loaded.error.messageText, '\n'));
    }
    const parsed = ts.parseJsonConfigFileContent(loaded.config, ts.sys, dirname(configPath), { noEmit: true }, configPath);
    if (parsed.errors.length) {
      throw new Error(parsed.errors.map((item) => ts.flattenDiagnosticMessageText(item.messageText, '\n')).join('\n'));
    }
    const program = ts.createProgram({ rootNames: parsed.fileNames, options: parsed.options });
    return { configPath, program, checker: program.getTypeChecker() };
  });
}

function canonicalSymbol(checker, symbol) {
  if (!symbol) return undefined;
  if (symbol.flags & ts.SymbolFlags.Alias) {
    try {
      return checker.getAliasedSymbol(symbol);
    } catch {
      return symbol;
    }
  }
  return symbol;
}

function declarationName(declaration) {
  if (declaration?.name && ts.isIdentifier(declaration.name)) return declaration.name.text;
  if (ts.isVariableDeclaration(declaration) && ts.isIdentifier(declaration.name)) return declaration.name.text;
  if (ts.isMethodSignature(declaration) || ts.isMethodDeclaration(declaration) || ts.isPropertySignature(declaration)) {
    return declaration.name?.getText() ?? '<method>';
  }
  if (declaration?.parent && ts.isVariableDeclaration(declaration.parent) && ts.isIdentifier(declaration.parent.name)) {
    return declaration.parent.name.text;
  }
  return '<anonymous>';
}

function symbolKey(checker, symbol) {
  const target = canonicalSymbol(checker, symbol);
  const declaration = target?.valueDeclaration ?? target?.declarations?.[0];
  if (!declaration) return undefined;
  return `${resolve(declaration.getSourceFile().fileName)}:${declaration.pos}:${target.getName()}`;
}

function callableDeclaration(declaration) {
  if (ts.isFunctionDeclaration(declaration) || ts.isClassDeclaration(declaration)) return true;
  if (ts.isVariableDeclaration(declaration)) {
    return Boolean(declaration.initializer && (ts.isArrowFunction(declaration.initializer) || ts.isFunctionExpression(declaration.initializer)));
  }
  return false;
}

function insideNode(candidate, ancestor) {
  return candidate.pos >= ancestor.pos && candidate.end <= ancestor.end && candidate.getSourceFile() === ancestor.getSourceFile();
}

function ignoredReference(node) {
  let current = node;
  while (current.parent) {
    current = current.parent;
    if (
      ts.isImportDeclaration(current) ||
      ts.isImportSpecifier(current) ||
      ts.isImportClause(current) ||
      ts.isNamespaceImport(current) ||
      ts.isExportDeclaration(current) ||
      ts.isExportSpecifier(current)
    ) return true;
    if (ts.isStatement(current) || ts.isDeclaration(current)) break;
  }
  return false;
}

function lineOf(sourceFile, node) {
  return sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile)).line + 1;
}

function scanTypeScriptSymbols(root, programs) {
  const findings = [];
  let filesScanned = 0;
  let symbolsScanned = 0;
  let referencesResolved = 0;

  for (const { program, checker } of programs) {
    const declarations = new Map();
    const projectFiles = program.getSourceFiles().filter((sourceFile) =>
      isInside(root, sourceFile.fileName) && !sourceFile.isDeclarationFile
    );
    filesScanned += projectFiles.length;

    for (const sourceFile of projectFiles.filter((item) => !isTestPath(item.fileName))) {
      const moduleSymbol = checker.getSymbolAtLocation(sourceFile);
      if (!moduleSymbol) continue;
      for (const exported of checker.getExportsOfModule(moduleSymbol)) {
        const target = canonicalSymbol(checker, exported);
        const declaration = target?.valueDeclaration ?? target?.declarations?.find(callableDeclaration);
        if (!declaration || declaration.getSourceFile() !== sourceFile || !callableDeclaration(declaration)) continue;
        const key = symbolKey(checker, target);
        if (!key || declarations.has(key)) continue;
        declarations.set(key, {
          key,
          declaration,
          sourceFile,
          symbol: target.getName(),
          productionFiles: new Set(),
          testFiles: new Set(),
          references: 0,
        });
      }
    }
    symbolsScanned += declarations.size;

    const visit = (sourceFile, node) => {
      if (ts.isIdentifier(node) && !ignoredReference(node)) {
        const key = symbolKey(checker, checker.getSymbolAtLocation(node));
        const tracked = key ? declarations.get(key) : undefined;
        if (tracked && !insideNode(node, tracked.declaration)) {
          const fileName = repoRelative(root, sourceFile.fileName);
          (isTestPath(sourceFile.fileName) ? tracked.testFiles : tracked.productionFiles).add(fileName);
          tracked.references += 1;
          referencesResolved += 1;
        }
      }
      ts.forEachChild(node, (child) => visit(sourceFile, child));
    };
    for (const sourceFile of projectFiles) visit(sourceFile, sourceFile);

    for (const tracked of declarations.values()) {
      if (tracked.productionFiles.size > 0 || tracked.testFiles.size === 0) continue;
      const file = repoRelative(root, tracked.sourceFile.fileName);
      findings.push({
        id: `orphan:${file}:${tracked.symbol}`,
        code: 'exported-test-only',
        file,
        line: lineOf(tracked.sourceFile, tracked.declaration),
        symbol: tracked.symbol,
        message: `production_importers=0 test_importers=${tracked.testFiles.size}; exported callable is reached only from tests/fixtures`,
      });
    }
  }
  return { findings, filesScanned, symbolsScanned, referencesResolved };
}

function isVoidLike(type) {
  if (type.flags & (ts.TypeFlags.Void | ts.TypeFlags.Undefined | ts.TypeFlags.Never | ts.TypeFlags.Any | ts.TypeFlags.Unknown)) return true;
  if (type.isUnion?.()) return type.types.every(isVoidLike);
  return false;
}

function expressionResultUsed(call) {
  let current = call;
  while (
    current.parent &&
    (ts.isParenthesizedExpression(current.parent) ||
      ts.isAsExpression(current.parent) ||
      ts.isTypeAssertionExpression(current.parent) ||
      ts.isNonNullExpression(current.parent) ||
      ts.isAwaitExpression(current.parent))
  ) current = current.parent;
  return !(current.parent && (ts.isExpressionStatement(current.parent) || ts.isVoidExpression(current.parent)));
}

function enclosingFunctionName(node) {
  let current = node.parent;
  while (current) {
    if (ts.isFunctionDeclaration(current) || ts.isMethodDeclaration(current) || ts.isFunctionExpression(current)) {
      if (current.name) return current.name.getText();
    }
    if (ts.isArrowFunction(current)) {
      if (ts.isVariableDeclaration(current.parent) && ts.isIdentifier(current.parent.name)) return current.parent.name.text;
      if (ts.isPropertyAssignment(current.parent)) return current.parent.name.getText();
      if (ts.isCallExpression(current.parent) && ts.isVariableDeclaration(current.parent.parent)) {
        return current.parent.parent.name.getText();
      }
    }
    current = current.parent;
  }
  return '<module>';
}

function returnedCallTarget(checker, declaration) {
  let body;
  if (ts.isVariableDeclaration(declaration) && declaration.initializer && (ts.isArrowFunction(declaration.initializer) || ts.isFunctionExpression(declaration.initializer))) {
    body = declaration.initializer.body;
  } else {
    body = declaration.body;
  }
  if (!body) return undefined;
  const targets = new Set();
  const visit = (node) => {
    if (ts.isReturnStatement(node) && node.expression && ts.isCallExpression(node.expression)) {
      const signature = checker.getResolvedSignature(node.expression);
      const targetDeclaration = signature?.declaration;
      if (targetDeclaration) targets.add(declarationName(targetDeclaration));
    }
    ts.forEachChild(node, visit);
  };
  visit(body);
  return targets.size === 1 ? [...targets][0] : undefined;
}

function scanDiscardedReturns(root, programs, config) {
  const groups = new Map();
  let callSitesScanned = 0;
  const externalPatterns = (config.typescript.returnValueDeclarationPatterns ?? []).map((value) => new RegExp(value));

  for (const { program, checker } of programs) {
    const sourceFiles = program.getSourceFiles().filter((sourceFile) =>
      isInside(root, sourceFile.fileName) && !sourceFile.isDeclarationFile && !isTestPath(sourceFile.fileName)
    );
    const visit = (sourceFile, node) => {
      if (ts.isCallExpression(node)) {
        const signature = checker.getResolvedSignature(node);
        const declaration = signature?.declaration;
        if (declaration) {
          const declarationFile = resolve(declaration.getSourceFile().fileName);
          const normalizedDeclaration = declarationFile.split(sep).join('/');
          const localDeclaration = isInside(root, declarationFile) && !normalizedDeclaration.includes('/node_modules/') && !declaration.getSourceFile().isDeclarationFile;
          const eligibleDeclaration = localDeclaration || externalPatterns.some((pattern) => pattern.test(normalizedDeclaration));
          const returnType = signature ? checker.getReturnTypeOfSignature(signature) : undefined;
          const promised = returnType ? checker.getPromisedTypeOfPromise(returnType) : undefined;
          if (eligibleDeclaration && promised && !isVoidLike(promised)) {
            callSitesScanned += 1;
            const key = `${declarationFile}:${declaration.pos}:${declarationName(declaration)}`;
            const group = groups.get(key) ?? {
              declaration,
              checker,
              calls: [],
              used: 0,
              discarded: 0,
            };
            const used = expressionResultUsed(node);
            group.calls.push({ sourceFile, node, used });
            group[used ? 'used' : 'discarded'] += 1;
            groups.set(key, group);
          }
        }
      }
      ts.forEachChild(node, (child) => visit(sourceFile, child));
    };
    for (const sourceFile of sourceFiles) visit(sourceFile, sourceFile);
  }

  const findings = [];
  for (const group of groups.values()) {
    if (group.calls.length === 0 || group.used > 0) continue;
    const first = group.calls[0];
    const declarationFile = group.declaration.getSourceFile();
    const declarationIsLocal = isInside(root, declarationFile.fileName);
    const file = declarationIsLocal ? repoRelative(root, declarationFile.fileName) : repoRelative(root, first.sourceFile.fileName);
    const baseName = declarationName(group.declaration);
    const forwarded = returnedCallTarget(group.checker, group.declaration);
    const symbol = forwarded ? `${baseName}->${forwarded}` : baseName;
    findings.push({
      id: `return:${file}:${baseName}`,
      code: 'return-discarded-everywhere',
      file,
      line: declarationIsLocal ? lineOf(declarationFile, group.declaration) : lineOf(first.sourceFile, first.node),
      symbol,
      message: `call_sites=${group.calls.length} used=0 discarded=${group.discarded}; Promise result is ignored at every production call site`,
    });
  }
  return { findings, callSitesScanned };
}

function suppressionHasReason(line) {
  return /--\s*\S|\s-\s+\S|reason:\s*\S/i.test(line);
}

function scanJsAndRustSuppressions(root, config) {
  const files = walk(root, config.suppressionRoots, new Set(['.ts', '.tsx', '.rs']));
  const findings = [];
  let suppressionsFound = 0;
  let rustFiles = 0;
  for (const path of files) {
    if (isTestPath(path)) continue;
    const extension = extname(path);
    if (extension === '.rs') rustFiles += 1;
    const lines = readFileSync(path, 'utf8').split(/\r?\n/);
    for (let index = 0; index < lines.length; index += 1) {
      const line = lines[index];
      let kind;
      if ((extension === '.ts' || extension === '.tsx') && /eslint-disable/.test(line)) kind = 'eslint-disable';
      if (extension === '.rs' && /#\s*\[\s*allow\s*\([^)]*dead_code[^)]*\)\s*\]/.test(line)) kind = 'allow-dead-code';
      if (!kind) continue;
      suppressionsFound += 1;
      const adjacentReason = suppressionHasReason(line) || (kind === 'allow-dead-code' && /reachability:\s*\S/i.test(lines[index - 1] ?? ''));
      if (adjacentReason) continue;
      const file = repoRelative(root, path);
      findings.push({
        id: `suppression:${file}:${index + 1}:${kind}`,
        code: 'suppression-missing-reason',
        file,
        line: index + 1,
        symbol: kind,
        message: `${kind} suppression has no adjacent justification after \`--\`, \` - \`, or \`reachability:\``,
      });
    }
  }
  return { findings, filesScanned: files.length, suppressionsFound, rustFiles };
}

function findProgramForFile(programs, path) {
  const wanted = resolve(path);
  for (const candidate of programs) {
    const sourceFile = candidate.program.getSourceFiles().find((item) => resolve(item.fileName) === wanted);
    if (sourceFile) return { ...candidate, sourceFile };
  }
  throw new Error(`TypeScript source is not included by a configured tsconfig: ${path}`);
}

function interfaceProperties(checker, sourceFile, name) {
  const declaration = sourceFile.statements.find((statement) => ts.isInterfaceDeclaration(statement) && statement.name.text === name);
  if (!declaration) throw new Error(`missing interface ${name} in ${sourceFile.fileName}`);
  const symbol = checker.getSymbolAtLocation(declaration.name);
  if (!symbol) throw new Error(`cannot resolve interface ${name} in ${sourceFile.fileName}`);
  return new Set(checker.getPropertiesOfType(checker.getDeclaredTypeOfSymbol(symbol)).map((item) => item.getName()));
}

function objectKeys(expression, keys = new Set()) {
  if (!expression) return keys;
  if (ts.isParenthesizedExpression(expression) || ts.isAsExpression(expression)) return objectKeys(expression.expression, keys);
  if (ts.isConditionalExpression(expression)) {
    objectKeys(expression.whenTrue, keys);
    objectKeys(expression.whenFalse, keys);
    return keys;
  }
  if (!ts.isObjectLiteralExpression(expression)) return keys;
  for (const property of expression.properties) {
    if (ts.isSpreadAssignment(property)) objectKeys(property.expression, keys);
    else if (property.name) keys.add(property.name.getText().replace(/^['"]|['"]$/g, ''));
  }
  return keys;
}

function findFactory(sourceFile, name) {
  const declaration = sourceFile.statements.find((statement) => ts.isFunctionDeclaration(statement) && statement.name?.text === name);
  if (!declaration) throw new Error(`missing factory ${name} in ${sourceFile.fileName}`);
  return declaration;
}

function serializedKeys(factory) {
  let result;
  const visit = (node) => {
    if (
      ts.isCallExpression(node) &&
      ts.isPropertyAccessExpression(node.expression) &&
      node.expression.expression.getText() === 'JSON' &&
      node.expression.name.text === 'stringify'
    ) result ??= objectKeys(node.arguments[0]);
    ts.forEachChild(node, visit);
  };
  visit(factory);
  if (!result) throw new Error(`no JSON.stringify object found in ${factory.name?.text ?? '<factory>'}`);
  return result;
}

function responseBodyReads(factory, variableName) {
  const fields = new Set();
  const visit = (node) => {
    if (ts.isPropertyAccessExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === variableName) {
      fields.add(node.name.text);
    }
    ts.forEachChild(node, visit);
  };
  visit(factory);
  return fields;
}

function wireFinding(name, side, kind, field, file, line, message) {
  return {
    id: `wire:${name}:${side}:${kind}:${field}`,
    code: 'wire-field-disconnected',
    file,
    line,
    symbol: field,
    message,
  };
}

function scanWireContracts(root, programs, config, pythonResult) {
  const findings = [];
  let fieldsScanned = 0;
  for (const contract of config.wireContracts ?? []) {
    const clientPath = join(root, contract.clientFile);
    const { checker, sourceFile } = findProgramForFile(programs, clientPath);
    const inputFields = interfaceProperties(checker, sourceFile, contract.inputInterface);
    const resultFields = interfaceProperties(checker, sourceFile, contract.resultInterface);
    const factory = findFactory(sourceFile, contract.factoryFunction);
    const serialized = serializedKeys(factory);
    const responseReads = responseBodyReads(factory, contract.responseVariable);
    const models = pythonResult.models[contract.name];
    const requestModel = models?.[contract.requestClass];
    const responseModel = models?.[contract.responseClass];
    if (!requestModel || !responseModel) throw new Error(`Python wire models missing for ${contract.name}`);
    const requestFields = new Set(requestModel.fields);
    const requiredRequestFields = new Set(requestModel.required);
    const responseFields = new Set(responseModel.fields);
    fieldsScanned += inputFields.size + serialized.size + responseFields.size + responseReads.size;
    const file = repoRelative(root, sourceFile.fileName);
    const line = lineOf(sourceFile, factory);

    for (const field of inputFields) if (!serialized.has(field)) {
      findings.push(wireFinding(contract.name, 'request', 'client_not_serialized', field, file, line, `client computes ${field} but the JSON body does not transmit it`));
    }
    for (const field of serialized) {
      if (!inputFields.has(field)) findings.push(wireFinding(contract.name, 'request', 'serialized_not_client', field, file, line, `JSON body transmits ${field} but the client input contract does not define it`));
      if (!requestFields.has(field)) findings.push(wireFinding(contract.name, 'request', 'serialized_not_server', field, file, line, `JSON body transmits ${field} but ${contract.requestClass} does not accept it`));
    }
    for (const field of requiredRequestFields) if (!serialized.has(field)) {
      findings.push(wireFinding(contract.name, 'request', 'required_server_missing', field, file, line, `${contract.requestClass}.${field} is required but absent from the JSON body`));
    }
    for (const field of responseFields) if (!responseReads.has(field)) {
      findings.push(wireFinding(contract.name, 'response', 'server_not_consumed', field, file, line, `${contract.responseClass} produces ${field} but the client never reads it`));
    }
    for (const field of responseReads) if (!responseFields.has(field)) {
      findings.push(wireFinding(contract.name, 'response', 'consumed_not_server', field, file, line, `client reads response.${field} but ${contract.responseClass} does not define it`));
    }
    for (const field of resultFields) if (!responseReads.has(field) && !responseFields.has(field)) {
      findings.push(wireFinding(contract.name, 'response', 'client_not_server', field, file, line, `client result exposes ${field} but neither reads nor receives it from the relay`));
    }
  }
  return { findings, fieldsScanned };
}

function literalValue(node) {
  if (ts.isNumericLiteral(node) && Number(node.text) === 0) return '0';
  if (node.kind === ts.SyntaxKind.FalseKeyword) return 'false';
  if (node.kind === ts.SyntaxKind.NullKeyword) return 'null';
  return undefined;
}

function scanCompositionLiterals(root, programs, config) {
  const findings = [];
  let parametersScanned = 0;
  const patterns = (config.composition.thresholdParameterPatterns ?? []).map((value) => new RegExp(value));
  for (const configuredFile of config.composition.roots ?? []) {
    const path = join(root, configuredFile);
    let source;
    try {
      source = findProgramForFile(programs, path);
    } catch {
      continue;
    }
    const { checker, sourceFile } = source;
    const visit = (node) => {
      if (ts.isCallExpression(node)) {
        const signature = checker.getResolvedSignature(node);
        const parameters = signature?.getParameters() ?? [];
        for (let index = 0; index < Math.min(parameters.length, node.arguments.length); index += 1) {
          const parameterName = parameters[index].getName();
          if (!patterns.some((pattern) => pattern.test(parameterName))) continue;
          parametersScanned += 1;
          const value = literalValue(node.arguments[index]);
          if (value === undefined) continue;
          const callee = node.expression.getText(sourceFile).split('.').at(-1);
          const enclosing = enclosingFunctionName(node);
          const file = repoRelative(root, sourceFile.fileName);
          findings.push({
            id: `literal:${file}:${enclosing}:${callee}:${parameterName}:${value}`,
            code: 'composition-disabling-literal',
            file,
            line: lineOf(sourceFile, node.arguments[index]),
            symbol: `${callee}.${parameterName}`,
            message: `composition root pins threshold/duration parameter ${parameterName}=${value}; this can make a branch unreachable`,
          });
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(sourceFile);
  }
  return { findings, parametersScanned };
}

function runPython(root, configPath, config) {
  const python = config.python.binary ?? 'python3';
  const configuredScript = config.python.script ?? 'tooling/reachability-python.py';
  const script = isAbsolute(configuredScript) ? configuredScript : join(root, configuredScript);
  const result = spawnSync(python, [script, '--root', root, '--config', configPath], {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 20 * 1024 * 1024,
  });
  if (result.status !== 0) throw new Error(`Python AST checker failed: ${(result.stderr || result.stdout).trim()}`);
  return JSON.parse(result.stdout);
}

function runRustClippy(root, config, skip) {
  const rust = config.rust ?? {};
  if (!rust.runClippy || skip) return { toolRuns: 0, findings: [], output: '' };
  const manifest = join(root, rust.cargoManifest);
  const target = process.env.ORB_REACHABILITY_CARGO_TARGET ?? '/tmp/orb-reachability-cargo-target';
  const args = ['clippy', '--manifest-path', manifest, '--locked', '--all-targets', '--all-features', '--offline', '--', '-D', 'dead-code'];
  const result = spawnSync('cargo', args, {
    cwd: root,
    encoding: 'utf8',
    env: { ...process.env, CARGO_TARGET_DIR: target },
    maxBuffer: 30 * 1024 * 1024,
  });
  const output = `${result.stdout ?? ''}${result.stderr ?? ''}`;
  if (result.status === 0) return { toolRuns: 1, findings: [], output };
  const firstError = output.split(/\r?\n/).find((line) => /^error(?:\[|:)/.test(line)) ?? 'cargo clippy failed';
  return {
    toolRuns: 1,
    output,
    findings: [{
      id: `rust-clippy:${rust.cargoManifest}`,
      code: 'rust-dead-code',
      file: rust.cargoManifest,
      line: 1,
      symbol: 'cargo-clippy',
      message: firstError,
    }],
  };
}

function applyAllowlist(findings, configured) {
  const allowlist = configured ?? [];
  const invalid = [];
  const byId = new Map();
  for (const entry of allowlist) {
    if (!entry.id || typeof entry.reason !== 'string' || entry.reason.trim().length < 12) {
      invalid.push({
        id: `allowlist-invalid:${entry.id ?? '<missing>'}`,
        code: 'allowlist-invalid',
        file: 'tooling/reachability.config.json',
        line: 1,
        symbol: entry.id ?? '<missing>',
        message: 'allowlist entries require a unique id and a one-line reason of at least 12 characters',
      });
    }
    if (byId.has(entry.id)) {
      invalid.push({
        id: `allowlist-duplicate:${entry.id}`,
        code: 'allowlist-invalid',
        file: 'tooling/reachability.config.json',
        line: 1,
        symbol: entry.id,
        message: 'allowlist id is duplicated',
      });
    }
    byId.set(entry.id, entry);
  }
  const matched = new Set();
  const active = [];
  const suppressed = [];
  for (const finding of findings) {
    const entry = byId.get(finding.id);
    if (entry) {
      matched.add(finding.id);
      suppressed.push({ ...finding, reason: entry.reason });
    } else active.push(finding);
  }
  for (const entry of allowlist) {
    if (entry.id && !matched.has(entry.id)) {
      invalid.push({
        id: `allowlist-stale:${entry.id}`,
        code: 'allowlist-stale',
        file: 'tooling/reachability.config.json',
        line: 1,
        symbol: entry.id,
        message: 'allowlist decision no longer matches a finding; remove or re-review it',
      });
    }
  }
  return { active: [...active, ...invalid], suppressed, entries: allowlist };
}

function makeZeroScanFinding(denominators) {
  if (denominators.sourceFiles > 0 && denominators.symbolsScanned > 0) return [];
  return [{
    id: 'census:zero-scan',
    code: 'zero-denominator',
    file: 'tooling/reachability.config.json',
    line: 1,
    symbol: 'census',
    message: `source_files=${denominators.sourceFiles} symbols_scanned=${denominators.symbolsScanned}; a run that scans nothing fails`,
  }];
}

export function runReachability(options) {
  const config = JSON.parse(readFileSync(options.config, 'utf8'));
  const programs = loadPrograms(options.root, config);
  const python = runPython(options.root, options.config, config);
  const tsSymbols = scanTypeScriptSymbols(options.root, programs);
  const returns = scanDiscardedReturns(options.root, programs, config);
  const suppressions = scanJsAndRustSuppressions(options.root, config);
  const wire = scanWireContracts(options.root, programs, config, python);
  const literals = scanCompositionLiterals(options.root, programs, config);
  const rust = runRustClippy(options.root, config, options.skipClippy);
  const denominators = {
    sourceFiles: tsSymbols.filesScanned + python.filesScanned,
    symbolsScanned: tsSymbols.symbolsScanned + python.symbolsScanned,
    importersResolved: tsSymbols.referencesResolved + python.referencesResolved,
    callSitesScanned: returns.callSitesScanned,
    wireFieldsScanned: wire.fieldsScanned,
    thresholdParametersScanned: literals.parametersScanned,
    suppressionsFound: suppressions.suppressionsFound + python.suppressionsFound,
    rustFilesScanned: suppressions.rustFiles,
    rustToolRuns: rust.toolRuns,
    allowlistSize: (config.allowlist ?? []).length,
  };
  const rawFindings = [
    ...tsSymbols.findings,
    ...python.findings,
    ...returns.findings,
    ...suppressions.findings,
    ...wire.findings,
    ...literals.findings,
    ...rust.findings,
    ...makeZeroScanFinding(denominators),
  ];
  const allowlisted = applyAllowlist(rawFindings, config.allowlist);
  const findings = allowlisted.active.sort((left, right) =>
    left.file.localeCompare(right.file) || left.line - right.line || left.code.localeCompare(right.code)
  );
  return {
    status: findings.length === 0 ? 'PASS' : 'FAIL',
    findings,
    allowlisted: allowlisted.suppressed,
    allowlist: allowlisted.entries,
    denominators,
    rustOutput: rust.output,
    tool: { name: 'ts-compiler-api+python-ast+cargo-clippy', typescript: ts.version },
  };
}

function printHuman(report) {
  const denominator = Object.entries(report.denominators).map(([key, value]) => `${key}=${value}`).join(' ');
  console.log(`reachability: ${report.status} findings=${report.findings.length} ${denominator}`);
  for (const finding of report.findings) {
    console.log(` - ${finding.code} ${finding.file}:${finding.line}:${finding.symbol} ${finding.message}`);
  }
  console.log(`reachability: allowlist=${report.allowlist.length}`);
  for (const entry of report.allowlist) console.log(` - ${entry.id} reason=${entry.reason}`);
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  try {
    const options = parseArgs(process.argv.slice(2));
    const report = runReachability(options);
    if (options.json) console.log(JSON.stringify(report, null, 2));
    else printHuman(report);
    process.exitCode = report.status === 'PASS' ? 0 : 1;
  } catch (error) {
    const message = error instanceof Error ? error.stack ?? error.message : String(error);
    console.error(`reachability: infrastructure failure\n${message}`);
    process.exitCode = 2;
  }
}
