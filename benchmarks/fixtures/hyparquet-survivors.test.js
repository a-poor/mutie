import { expect, it } from 'vitest'
import { matchFilter } from '../src/filter.js'
import { equals } from '../src/utils.js'

it('defaults to strict equality when strict is omitted', () => {
  expect(equals(1, '1')).toBe(false)
  expect(equals({ value: 1 }, { value: '1' })).toBe(false)
  expect(equals(1, '1', false)).toBe(true)
})

it('distinguishes byte arrays with equal prefixes but different lengths', () => {
  const short = new Uint8Array([1])
  const long = new Uint8Array([1, 2])
  expect(equals(short, long)).toBe(false)
  expect(equals(long, short)).toBe(false)
})

it('does not match binary query values with different lengths', () => {
  const record = { bytes: new Uint8Array([1]) }
  expect(matchFilter(record, { bytes: { $eq: new Uint8Array([1, 2]) } })).toBe(false)
  expect(matchFilter(record, { bytes: { $in: [new Uint8Array([1, 2])] } })).toBe(false)
  expect(matchFilter(record, { bytes: { $eq: new Uint8Array([1]) } })).toBe(true)
})
