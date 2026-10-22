import {test, expect} from 'bun:test';
import isIPv4 from '../src/main/isIPv4.js';

test('IPv4 accepts both valid octet endpoints in every position', () => {
  for (let position = 0; position < 4; position++) {
    for (const endpoint of [0, 255]) {
      const octets = [1, 2, 3, 4];
      octets[position] = endpoint;
      expect(isIPv4(octets.join('.'))).toBe(true);
    }
  }
});
