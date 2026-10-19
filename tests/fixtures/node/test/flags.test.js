import { test } from 'node:test'
import assert from 'node:assert/strict'
import { enabled, disabled, fallback } from '../src/flags.js'
test('enabled', () => assert.equal(enabled(), true))
test('disabled', () => assert.equal(disabled(), false))
test('fallback with truthy input', () => assert.equal(fallback(true), true))
