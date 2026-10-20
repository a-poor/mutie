import { test, expect } from 'bun:test';
import is from '../src/pp-is.js';

// These predicates document string inputs. Numeric-looking non-strings must
// not bypass the string guard and become successful matches.
for (const [name, value] of [
  ['isAlphanumeric', 123],
  ['isHexColor', 123],
  ['isIPv4', 123],
  ['isLowercase', 123],
  ['isUUID', 123],
  ['isUppercase', 123],
]) {
  test(`${name} rejects non-string input`, () => {
    expect(is[name](value)).toBe(false);
  });
}

test('IPv4 rejects a nonnumeric octet even when the other octets are valid', () => {
  expect(is.isIPv4('1.2.3.x')).toBe(false);
  expect(is.isIPv4('1.2..4')).toBe(false);
});
