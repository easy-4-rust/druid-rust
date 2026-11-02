import { test } from 'node:test'
import assert from 'node:assert/strict'
import { api } from '../src/api/client.js'

test('wire defaults are masked by field availability without erasing real zero', async () => {
  const previous = globalThis.fetch
  globalThis.fetch = async () => new Response(JSON.stringify({ datasources: [{ activeCount: 0, errorCount: 0, unavailableFields: ['errorCount'] }] }))
  try {
    const body = await api.datasources()
    assert.equal(body.datasources[0].activeCount, 0)
    assert.equal(body.datasources[0].errorCount, null)
  } finally { globalThis.fetch = previous }
})

test('detail request preserves and encodes source scope', async () => {
  const previous = globalThis.fetch
  let requested
  globalThis.fetch = async (url) => { requested = url; return new Response('{}') }
  try {
    await api.sqlDetail(7, { serviceId: 'service/a+b', datasourceId: 4 })
    const url = new URL(requested, 'http://localhost')
    assert.equal(url.searchParams.get('serviceId'), 'service/a+b')
    assert.equal(url.searchParams.get('datasourceId'), '4')
  } finally { globalThis.fetch = previous }
})
