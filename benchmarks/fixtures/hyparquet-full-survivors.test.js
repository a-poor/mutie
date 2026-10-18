import { expect, it } from 'vitest'
import { DEFAULT_PARSERS } from '../src/convert.js'
import { decodeVariantColumn } from '../src/variant.js'
import { getSchemaPath, isFlatColumn, isListLike, isMapLike } from '../src/schema.js'
import { parquetQuery } from '../src/query.js'
import { parquetReadObjects } from '../src/read.js'
import { asyncBufferFromFile } from '../src/node.js'
import { parquetPlanGroup, parquetPlanGroups, prefetchBloomFilters, prefetchPageIndexes } from '../src/plan.js'
import { convertMetadata, parquetMetadataAsync } from '../src/metadata.js'
import { canSkipRowGroup, filterPageRanges, matchFilter } from '../src/filter.js'

// Defaults must agree with the explicit strict mode; loose mode is opt-in.
it('defaults row filtering to strict equality', () => {
  expect(matchFilter({ x: 1 }, { x: { $eq: '1' } })).toBe(false)
  expect(matchFilter({ x: 1 }, { x: { $eq: '1' } }, false)).toBe(true)
  expect(matchFilter({ x: 1 }, { x: { $eq: 1 } })).toBe(true)
})

it('does not prune strictly unequal values by coercing the filter target', () => {
  const rowGroup = {
    num_rows: 3n,
    total_byte_size: 12n,
    columns: [{
      file_offset: 4n,
      meta_data: {
        type: 'INT32', path_in_schema: ['x'], codec: 'UNCOMPRESSED',
        num_values: 3n, total_compressed_size: 12n, total_uncompressed_size: 12n,
        encodings: ['PLAIN'], data_page_offset: 4n,
        statistics: { min_value: 1, max_value: 1, null_count: 0n },
      },
    }],
  }
  const options = { rowGroup, physicalColumns: ['x'], filter: { x: { $ne: '1' } } }
  expect(canSkipRowGroup(options)).toBe(false)
  expect(canSkipRowGroup({ ...options, strict: true })).toBe(false)
  expect(canSkipRowGroup({ ...options, strict: false })).toBe(true)
})

it('defaults page pruning to strict equality', () => {
  const pages = {
    x: { minValues: [1], maxValues: [1], nullPages: [false], nullCounts: [0n], pageStarts: [0] },
  }
  const filter = { x: { $ne: '1' } }
  expect(filterPageRanges(filter, pages, 3)).toEqual([[0, 3]])
  expect(filterPageRanges(filter, pages, 3, true)).toEqual([[0, 3]])
  expect(filterPageRanges(filter, pages, 3, false)).toEqual([])
})

it('keeps pages whose value bounds are missing', () => {
  for (const [min, max] of [[undefined, 1], [1, undefined]]) {
    const pages = {
      x: { minValues: [min], maxValues: [max], nullPages: [false], nullCounts: [0n], pageStarts: [0] },
    }
    expect(filterPageRanges({ x: { $eq: 2 } }, pages, 3)).toEqual([[0, 3]])
  }
  const bounded = {
    x: { minValues: [1], maxValues: [1], nullPages: [false], nullCounts: [0n], pageStarts: [0] },
  }
  expect(filterPageRanges({ x: { $eq: 2 } }, bounded, 3)).toEqual([])
})

it('decodes nonzero little-endian date statistics', () => {
  expect(convertMetadata(new Uint8Array([1, 0, 0, 0]), {
    name: 'day', type: 'INT32', converted_type: 'DATE',
  }, DEFAULT_PARSERS)).toEqual(new Date('1970-01-02T00:00:00.000Z'))
})

it.each([
  ['legacy milliseconds', { converted_type: 'TIMESTAMP_MILLIS' }, 1234n],
  ['legacy microseconds', { converted_type: 'TIMESTAMP_MICROS' }, 1234000n],
  ['logical nanoseconds', { logical_type: { type: 'TIMESTAMP', unit: 'NANOS', isAdjustedToUTC: true } }, 1234000000n],
  ['logical microseconds', { logical_type: { type: 'TIMESTAMP', unit: 'MICROS', isAdjustedToUTC: true } }, 1234000n],
  ['logical milliseconds', { logical_type: { type: 'TIMESTAMP', unit: 'MILLIS', isAdjustedToUTC: true } }, 1234n],
])('decodes nonzero little-endian %s statistics', (_label, annotation, value) => {
  const bytes = new Uint8Array(8)
  new DataView(bytes.buffer).setBigInt64(0, value, true)
  expect(convertMetadata(bytes, { name: 'time', type: 'INT64', ...annotation }, DEFAULT_PARSERS))
    .toEqual(new Date('1970-01-01T00:00:01.234Z'))
})

async function indexedMetadata() {
  const file = await asyncBufferFromFile('test/files/offset_indexed.parquet')
  const metadata = await parquetMetadataAsync(file)
  return { file, metadata }
}

it('defaults group planning to strict filtering', async () => {
  const { metadata } = await indexedMetadata()
  metadata.row_groups[0].columns[0].meta_data.statistics = {
    min_value: 1, max_value: 1, null_count: 0n,
  }
  const options = { metadata, rowEnd: 100, filter: { id: { $ne: '1' } } }
  expect(parquetPlanGroups(options).groups).toHaveLength(1)
  expect(parquetPlanGroups({ ...options, filterStrict: false }).groups).toHaveLength(0)
})

it('does not request offset indexes by default in direct group planning', async () => {
  const { metadata } = await indexedMetadata()
  const options = { rowGroup: metadata.row_groups[0], groupStart: 0, groupRows: 100, ranges: [[10, 20]] }
  expect(parquetPlanGroup(options).indexes).toEqual([])
  expect(parquetPlanGroup({ ...options, useOffsetIndex: true }).indexes.length).toBeGreaterThan(0)
})

it('coalesces disjoint selections when page locations are unknown', async () => {
  const { metadata } = await indexedMetadata()
  const options = { metadata, rowEnd: 100, columns: ['content'], pageRangesByGroup: [[[0, 10], [20, 30]]] }
  const plan = parquetPlanGroups(options)
  expect(plan.groups[0].ranges).toEqual([[0, 30]])
  const known = { content: [
    { offset: 438n, compressed_page_size: 100, first_row_index: 0n },
    { offset: 538n, compressed_page_size: 100, first_row_index: 15n },
  ] }
  expect(parquetPlanGroups({ ...options, pageLocationsByGroup: [known] }).groups[0].ranges)
    .toEqual([[0, 10], [20, 30]])
})

it('does not fetch output indexes when no filter column has page statistics', async () => {
  const { file: source, metadata } = await indexedMetadata()
  for (const group of metadata.row_groups) {
    for (const chunk of group.columns) {
      delete chunk.column_index_offset
      delete chunk.column_index_length
    }
  }
  let reads = 0
  const file = { byteLength: source.byteLength, slice(...args) { reads++; return source.slice(...args) } }
  const result = await prefetchPageIndexes({ file, metadata, filter: { id: { $eq: 10 } }, columns: ['content'] })
  expect(reads).toBe(0)
  expect(result.pageRangesByGroup).toEqual([undefined, undefined])
})

it('fetches bloom filters using strict mode unless loose mode is requested', async () => {
  const { metadata } = await indexedMetadata()
  metadata.row_groups = [metadata.row_groups[0]]
  const meta = metadata.row_groups[0].columns[0].meta_data
  meta.statistics = { min_value: 1, max_value: 1, null_count: 0n }
  // Valid compact-Thrift header for one uncompressed 32-byte split block.
  const bytes = new Uint8Array([0x15, 0x40, 0x1c, 0x1c, 0, 0, 0x1c, 0x1c, 0, 0, 0x1c, 0x1c, 0, 0, 0, ...Array(32).fill(0)])
  meta.bloom_filter_offset = 4n
  meta.bloom_filter_length = bytes.length
  let reads = 0
  const file = { byteLength: bytes.length + 4, slice() { reads++; return bytes.buffer } }
  const filter = { $and: [{ id: { $eq: 1 } }, { id: { $ne: '1' } }] }
  const result = await prefetchBloomFilters({ file, metadata, filter })
  expect(result[0].id.numBytes).toBe(32)
  expect(reads).toBe(1)
  expect(await prefetchBloomFilters({ file, metadata, filter, filterStrict: false })).toEqual([{}])
  expect(reads).toBe(1)
})

it('does not silently skip a required page-index request by coercing a filter', async () => {
  const { metadata } = await indexedMetadata()
  metadata.row_groups = [metadata.row_groups[0]]
  const chunk = metadata.row_groups[0].columns[0]
  chunk.meta_data.statistics = { min_value: 1, max_value: 1, null_count: 0n }
  chunk.column_index_offset = 4n
  chunk.column_index_length = 1
  chunk.offset_index_offset = 8n
  chunk.offset_index_length = 1
  const fetchError = new Error('page index request reached the backing store')
  const file = { byteLength: 100000, slice() { throw fetchError } }
  const options = { file, metadata, filter: { id: { $ne: '1' } } }
  await expect(prefetchPageIndexes(options)).rejects.toBe(fetchError)
  await expect(prefetchPageIndexes({ ...options, filterStrict: false })).resolves.toMatchObject({ pageRangesByGroup: [undefined] })
})

it('defaults the public read API to strict filtering', async () => {
  const file = await asyncBufferFromFile('test/files/datapage_v2.snappy.parquet')
  const options = { file, columns: ['b'], filter: { b: { $eq: '1' } } }
  expect(await parquetReadObjects(options)).toEqual([])
  expect(await parquetReadObjects({ ...options, filterStrict: false })).toEqual([{ b: 1 }])
})

it('does not fetch projected data from unselected groups in a sorted query', async () => {
  const { file: source, metadata } = await indexedMetadata()
  const reads = []
  const file = { byteLength: source.byteLength, slice(start, end) {
    reads.push([start, end ?? source.byteLength])
    return source.slice(start, end)
  } }
  const rows = await parquetQuery({ file, metadata, columns: ['content'], orderBy: 'id', rowStart: 150, rowEnd: 151 })
  expect(rows).toHaveLength(1)
  expect(rows[0].__index__).toBe(150)
  const content = metadata.row_groups[0].columns[1].meta_data
  const start = Number(content.dictionary_page_offset || content.data_page_offset)
  const end = start + Number(content.total_compressed_size)
  expect(reads.some(([a, b]) => a < end && b > start)).toBe(false)
})

it('requires a repeated list container', () => {
  const schema = [
    { name: 'root', num_children: 1 },
    { name: 'items', converted_type: 'LIST', repetition_type: 'OPTIONAL', num_children: 1 },
    { name: 'list', repetition_type: 'OPTIONAL', num_children: 1 },
    { name: 'element', type: 'INT32', repetition_type: 'REQUIRED' },
  ]
  expect(isListLike(getSchemaPath(schema, ['items'])[1])).toBe(false)
  schema[2].repetition_type = 'REPEATED'
  expect(isListLike(getSchemaPath(schema, ['items'])[1])).toBe(true)
})

it('rejects invalid map container and key/value repetition shapes', () => {
  const schema = [
    { name: 'root', num_children: 1 },
    { name: 'map', converted_type: 'MAP', repetition_type: 'OPTIONAL', num_children: 1 },
    { name: 'key_value', repetition_type: 'REPEATED', num_children: 2 },
    { name: 'key', type: 'INT32', repetition_type: 'REQUIRED' },
    { name: 'value', type: 'INT32', repetition_type: 'OPTIONAL' },
  ]
  const valid = getSchemaPath(schema, ['map'])[1]
  expect(isMapLike(valid)).toBe(true)
  const extraContainer = structuredClone(valid)
  extraContainer.children.push(structuredClone(valid.children[0]))
  expect(isMapLike(extraContainer)).toBe(false)
  const missingValue = structuredClone(valid)
  missingValue.children[0].children.pop()
  expect(isMapLike(missingValue)).toBe(false)
  for (const target of ['container', 'key', 'value']) {
    const invalid = structuredClone(valid)
    if (target === 'container') invalid.children[0].element.repetition_type = 'OPTIONAL'
    else invalid.children[0].children.find(child => child.element.name === target).element.repetition_type = 'REPEATED'
    expect(isMapLike(invalid)).toBe(false)
  }
})

it('distinguishes flat leaves from nested, repeated, and group columns', () => {
  const schema = [
    { name: 'root', num_children: 3 },
    { name: 'flat', type: 'INT32', repetition_type: 'OPTIONAL' },
    { name: 'repeated', type: 'INT32', repetition_type: 'REPEATED' },
    { name: 'group', repetition_type: 'OPTIONAL', num_children: 1 },
    { name: 'child', type: 'INT32', repetition_type: 'OPTIONAL' },
  ]
  expect(isFlatColumn(getSchemaPath(schema, ['flat']))).toBe(true)
  expect(isFlatColumn(getSchemaPath(schema, ['repeated']))).toBe(false)
  expect(isFlatColumn(getSchemaPath(schema, ['group']))).toBe(false)
  expect(isFlatColumn(getSchemaPath(schema, ['group', 'child']))).toBe(false)
})

function decodePrimitive(tag, payload) {
  return decodeVariantColumn({
    metadata: new Uint8Array([0x11, 0, 0]),
    value: new Uint8Array([tag << 2, ...payload]),
  })
}

it.each([
  ['int64', 6, 'setBigInt64', 123456789n, 8, 123456789n],
  ['double', 7, 'setFloat64', 1.5, 8, 1.5],
  ['timestamp micros', 12, 'setBigInt64', 1234000n, 8, new Date('1970-01-01T00:00:01.234Z')],
  ['float', 14, 'setFloat32', 1.5, 4, 1.5],
  ['time micros', 17, 'setBigInt64', 123456789n, 8, 123456789n],
  ['timestamp nanos', 18, 'setBigInt64', 1234000000n, 8, new Date('1970-01-01T00:00:01.234Z')],
])('decodes nonzero little-endian variant %s', (_label, tag, setter, value, width, expected) => {
  const bytes = new Uint8Array(width)
  new DataView(bytes.buffer)[setter](0, value, true)
  expect(decodePrimitive(tag, bytes)).toEqual(expected)
})

it('decodes both halves of a nonzero 128-bit variant decimal', () => {
  const bytes = new Uint8Array(17)
  bytes[0] = 2 // scale
  const view = new DataView(bytes.buffer)
  view.setBigUint64(1, 12345n, true)
  view.setBigInt64(9, 1n, true)
  expect(decodePrimitive(10, bytes)).toBe(Number((1n << 64n) + 12345n) * 0.01)
})

it('decodes a nonzero 64-bit variant decimal', () => {
  const bytes = new Uint8Array(9)
  bytes[0] = 2
  new DataView(bytes.buffer).setBigInt64(1, 12345n, true)
  expect(decodePrimitive(9, bytes)).toBe(123.45)
})

it('decodes a nonzero 32-bit variant decimal', () => {
  const bytes = new Uint8Array(5)
  bytes[0] = 2
  new DataView(bytes.buffer).setInt32(1, 12345, true)
  expect(decodePrimitive(8, bytes)).toBe(123.45)
})
