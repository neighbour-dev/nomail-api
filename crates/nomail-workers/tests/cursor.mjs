import assert from 'node:assert/strict';

export async function testMessageCursors(api, localSql, token) {
  const config = await api('POST', '/api/webhook/configs', { name: 'Cursor queries', platform: { provider: 'generic', credentials: { type: 'none' } } }, token);
  const base = `/api/messages/cursor?webhookConfigId=${config.id}`;
  const empty = await api('GET', base, undefined, token);
  assert.deepEqual(empty, { items: [], hasMore: false, nextCursor: null, startCursor: null, endCursor: null });
  const ids = [];
  for (let i = 0; i < 9; i++) {
    const created = await api('POST', config.url, { title: 'Cursor', content: 'hello\n world', level: 'high' });
    ids.push(created.messageId);
  }
  localSql(`UPDATE notify_messages SET created_at='2040-01-01T00:00:00.123Z' WHERE webhook_config_id=${config.id}; UPDATE notify_messages SET created_at='2039-12-31T23:59:59.123Z' WHERE id='${ids[0]}'; UPDATE notify_messages SET created_at='2040-01-01T00:00:01.123Z' WHERE id='${ids[8]}';`);
  const expected = [ids[0], ...ids.slice(1, 8).sort(), ids[8]];
  let path = base + '&limit=3';
  const history = [];
  const pages = [];
  for (;;) {
    const page = await api('GET', path, undefined, token);
    for (const item of page.items) assert.equal(item.summary, 'hello world');
    history.push(...page.items.map(item => item.id));
    pages.push(page);
    assert.equal(page.total, undefined);
    assert.equal(page.page, undefined);
    if (!page.hasMore) { assert.equal(page.nextCursor, null); break; }
    assert.equal(page.nextCursor, page.endCursor);
    path = `${base}&limit=3&before=${page.nextCursor}`;
  }
  assert.deepEqual(history, expected.toReversed());
  path = `${base}&limit=2&after=${pages[2].startCursor}`;
  const added = [];
  for (;;) {
    const page = await api('GET', path, undefined, token);
    added.push(...page.items.map(item => item.id));
    if (!page.hasMore) break;
    path = `${base}&limit=2&after=${page.nextCursor}`;
  }
  assert.deepEqual(added, expected.slice(3));
  assert.deepEqual(await api('GET', `${base}&after=${pages[0].startCursor}`, undefined, token), empty);
  await api('DELETE', `/api/messages/${expected[6]}`, undefined, token);
  assert.deepEqual((await api('GET', `${base}&before=${pages[0].endCursor}`, undefined, token)).items.map(item => item.id), expected.slice(0, 6).toReversed());
  assert.deepEqual((await api('GET', `${base}&after=${pages[0].endCursor}`, undefined, token)).items.map(item => item.id), expected.slice(7));
  for (const query of ['status=read', 'level=low', 'pinned=true', 'query=missing', 'from=2041-01-01T00:00:00Z', 'to=2039-01-01T00:00:00Z']) {
    assert.deepEqual((await api('GET', `${base}&${query}`, undefined, token)).items, [], query);
  }
  assert.equal((await api('GET', `${base}&query=world&status=unread&level=HIGH&pinned=false&from=2040-01-01T00:00:00Z&to=2040-01-01T00:00:01Z`, undefined, token)).items.length, 6);
  for (const query of ['before=x&after=y', 'before=', 'after=bad', 'limit=0', 'limit=101', 'limit=no', 'from=bad', 'status=no', 'level=no', 'pinned=no', 'webhookConfigId=0', 'from=2041-01-01T00:00:00Z&to=2040-01-01T00:00:00Z']) {
    await api('GET', `/api/messages/cursor?${query}`, undefined, token, 400);
  }
  // The millisecond boundary is reconstructed in the exact D1 storage format.
  const decoded = JSON.parse(Buffer.from(pages[1].startCursor, 'base64url').toString());
  assert.equal(decoded.receivedAt, '2040-01-01T00:00:00.123Z');
  const subMs = Buffer.from(JSON.stringify({ ...decoded, receivedAt: '2040-01-01T00:00:00.123000001Z' })).toString('base64url');
  assert.deepEqual((await api('GET', `${base}&after=${subMs}`, undefined, token)).items.map(item => item.id), [expected[8]]);
  assert.equal((await api('GET', `${base}&before=${subMs}`, undefined, token)).items.length, 7);
  const indexes = localSql("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_messages_active_%';");
  assert.deepEqual(indexes.map(row => row.name).sort(), ['idx_messages_active_config_time_id', 'idx_messages_active_time_id']);
  for (const [operator, order] of [['<', 'DESC'], ['>', 'ASC']]) {
    const rows = localSql(`EXPLAIN QUERY PLAN SELECT id,title,summary FROM notify_messages WHERE deleted_at IS NULL AND webhook_config_id=${config.id} AND (created_at,id)${operator}('2040-01-01T00:00:00.123Z','${expected[4]}') ORDER BY created_at ${order},id ${order} LIMIT 3;`);
    const plan = rows.map(row => row.detail).join('\n');
    assert.match(plan, /idx_messages_active_config_time_id/);
    assert.doesNotMatch(plan, /TEMP B-TREE/);
  }
  const owner = localSql(`SELECT user_id FROM webhook_configs WHERE id=${config.id};`)[0].user_id;
  localSql(`UPDATE webhook_configs SET user_id=${owner + 1000} WHERE id=${config.id};`);
  assert.deepEqual((await api('GET', base, undefined, token)).items, []);
  assert.ok(!(await api('GET', '/api/messages/cursor', undefined, token)).items.some(item => item.webhookConfigId === config.id));
  localSql(`UPDATE webhook_configs SET user_id=${owner} WHERE id=${config.id};`);
  await api('DELETE', `/api/webhook/configs/${config.id}`, undefined, token);
  assert.deepEqual((await api('GET', base, undefined, token)).items, []);
}
