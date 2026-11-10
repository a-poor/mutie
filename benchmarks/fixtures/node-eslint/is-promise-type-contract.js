import test from 'node:test';
import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';
import ts from 'typescript';
import isPromiseType from '../../rules/utils/is-promise-type.js';

const filename = fileURLToPath(new URL('promise-unions.ts', import.meta.url));
const source = 'type Promises = Promise<string> | Promise<number>;\ntype Values = string | number;\ntype Mixed = Promise<string> | number;';
const options = {
	strict: true, noEmit: true, types: [], target: ts.ScriptTarget.ES2022,
};
const host = ts.createCompilerHost(options);
const getSourceFile = host.getSourceFile.bind(host);
host.getSourceFile = (path, ...arguments_) => path === filename
	? ts.createSourceFile(path, source, ts.ScriptTarget.ES2022, true)
	: getSourceFile(path, ...arguments_);
const program = ts.createProgram([filename], options, host);
const checker = program.getTypeChecker();
const types = new Map(program.getSourceFile(filename).statements
	.filter(node => ts.isTypeAliasDeclaration(node))
	.map(node => [node.name.text, checker.getTypeFromTypeNode(node.type)]));

test('all-promise unions remain promises', () => {
	const type = types.get('Promises');
	assert.equal(type.isUnion(), true);
	assert.equal(isPromiseType(type, checker), true);
});

test('all-value unions remain non-promises', () => {
	const type = types.get('Values');
	assert.equal(type.isUnion(), true);
	assert.equal(isPromiseType(type, checker), false);
});

test('mixed unions remain indeterminate', () => {
	const type = types.get('Mixed');
	assert.equal(type.isUnion(), true);
	assert.equal(isPromiseType(type, checker), undefined);
});
