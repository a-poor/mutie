import { test } from 'node:test';
import assert from 'node:assert/strict';
import { enabled, disabled, fallback } from '../src/flags.ts';
import type { Flag } from '../src/types.ts';
// Native stripping does not type-check: this deliberately invalid assignment runs.
const runtime: true = false;
test('enabled', (): void => assert.equal(enabled(), true));
test('disabled', (): void => assert.equal(disabled(), runtime));
test('fallback with truthy input', () => assert.equal(fallback<Flag>(true), true));
