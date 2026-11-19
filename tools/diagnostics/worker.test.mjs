import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import { DatabaseSync } from 'node:sqlite';
import { test } from 'node:test';
import worker from './worker.mjs';

test('stores only approved aggregates and expires old counts', async () => {
  const db = new DatabaseSync(':memory:');
  db.exec(readFileSync(new URL('schema.sql', import.meta.url), 'utf8'));
  const env = {
    REPORTS: {
      prepare(sql) {
        const statement = db.prepare(sql);
        return { run: () => statement.run(), bind: (...args) => ({ run: () => statement.run(...args) }) };
      },
    },
  };
  const url = 'https://local.invalid/v1/reports';
  const good = { version: '0.1.2', os: 'windows', event: 'game_crash', detail: 'device_lost', fps: null };
  const post = (event) => worker.fetch(new Request(url, {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(event),
  }), env);
  assert.equal((await post(good)).status, 204);
  assert.equal((await post(good)).status, 204);
  assert.equal((await post({ ...good, log: 'C:\\Users\\Private' })).status, 400);
  assert.equal((await post({ ...good, detail: 'C:\\Users\\Private' })).status, 400);
  assert.equal((await post({ ...good, detail: '*'.repeat(1100) })).status, 400);
  assert.equal((await worker.fetch(new Request(url), env)).status, 404);
  assert.equal(db.prepare('SELECT total FROM reports').get().total, 2);
  db.exec("UPDATE reports SET day = '2000-01-01'");
  await worker.scheduled({}, env);
  assert.equal(db.prepare('SELECT count(*) AS rows FROM reports').get().rows, 0);
  db.close();
});
