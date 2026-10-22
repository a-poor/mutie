import {test, expect} from 'bun:test';
import isBoolean from '../src/main/isBoolean.js';

test('Boolean objects are accepted alongside primitive booleans', () => {
  expect(isBoolean(new Boolean(true))).toBe(true);
  expect(isBoolean(new Boolean(false))).toBe(true);
});
