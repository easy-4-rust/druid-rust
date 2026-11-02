import {test} from 'node:test'
import assert from 'node:assert/strict'
import {createRequestGuard} from '../src/utils/requestGuard.js'

test('a late refresh cannot replace the newest result and unmount invalidates pending results', async () => {
 const guard = createRequestGuard()
 let value = 'initial', resolveOld
 const old = guard.begin()
 const pending = new Promise(resolve => {resolveOld = resolve}).then(result => {if(old()) value=result})
 const current = guard.begin()
 if(current()) value = 'new result'
 resolveOld('stale result')
 await pending
 assert.equal(value, 'new result')
 guard.dispose()
 assert.equal(current(), false)
 assert.equal(guard.begin()(), false)
})
