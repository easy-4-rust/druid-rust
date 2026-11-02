import { test } from 'node:test'
import assert from 'node:assert/strict'
import { formatNumber, formatDuration, zipBuckets } from '../src/utils/format.js'

test('missing observations must not look like measured zero', () => {
  for (const value of [null, undefined, '', NaN]) {
    assert.equal(formatNumber(value), '未上报')
    assert.equal(formatDuration(value), '未上报')
  }
  assert.equal(formatNumber(0), '0')
  assert.equal(formatDuration(0), '0 ms')
})

test('histogram zero and unavailable distributions remain distinct', () => {
  assert.deepEqual(zipBuckets(null, ['a']), [])
  assert.deepEqual(zipBuckets([0], ['a']), [{ label: 'a', count: 0 }])
  assert.equal(formatDuration(1500), '1.50 s')
})
