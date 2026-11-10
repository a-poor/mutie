import test from 'node:test';
import assert from 'node:assert/strict';
import {Linter} from 'eslint';
import plugin from '../../index.js';

test('reports an assertion comparing the same this value', () => {
	const linter = new Linter();
	const messages = linter.verify(
		'import assert from \'node:assert\'; class Subject { check() { assert.strictEqual(this, this); } }',
		{
			plugins: {'node-test': plugin},
			rules: {'node-test/no-identical-assertion-arguments': 'error'},
		},
		{filename: 'subject.js'},
	);
	assert.equal(messages.length, 1);
	assert.equal(messages[0].messageId, 'no-identical-assertion-arguments/always-passes');
});
