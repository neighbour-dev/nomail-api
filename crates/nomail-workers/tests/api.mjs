import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHmac, createHash, createCipheriv } from 'node:crypto';
import { validateResponse, validateSse, protectedRoutes } from './contract.mjs';
import { testMessageCursors } from './cursor.mjs';

test('Workers API on real local D1 and Durable Objects', { timeout: 180_000 }, async () => {
  const state = await mkdtemp(join(tmpdir(), 'nomail-worker-test-'));
  let child;
  let stopped;
  let logs = '';
  const origin = 'http://127.0.0.1:8791';
  try {
    const migration = spawnSync('wrangler', ['d1', 'migrations', 'apply', 'DB', '--local', '--persist-to', state], { encoding: 'utf8', env: { ...process.env, CI: 'true' } });
    assert.equal(migration.status, 0, migration.stdout + migration.stderr);
    child = spawn('wrangler', ['dev', '--local', '--ip', '127.0.0.1', '--port', '8791', '--persist-to', state,
      '--var', 'JWT_SECRET:local-test-secret-never-use-in-production',
      '--var', `SECRET_ENCRYPTION_KEY:${Buffer.alloc(32, 7).toString('base64')}`,
      '--var', 'INITIAL_USERNAME:admin', '--var', 'INITIAL_PASSWORD:password123'], { env: { ...process.env, CI: 'true' }, stdio: ['ignore', 'pipe', 'pipe'] });
    stopped = new Promise(resolve => child.once('exit', resolve));
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`Worker startup timed out\n${logs}`)), 120_000);
      const onData = data => {
        logs += data;
        if (logs.includes('Ready on http://127.0.0.1:8791')) { clearTimeout(timer); resolve(); }
      };
      child.stdout.on('data', onData);
      child.stderr.on('data', onData);
      child.once('error', error => { clearTimeout(timer); reject(error); });
      child.once('exit', code => { clearTimeout(timer); reject(new Error(`Worker exited ${code}\n${logs}`)); });
    });
    async function api(method, path, body, token, expected = 200) {
      const response = await fetch(origin + path, { method, headers: { 'Content-Type': 'application/json', ...(token ? { Authorization: `Bearer ${token}` } : {}) }, ...(body === undefined ? {} : { body: JSON.stringify(body) }) });
      const data = await response.json();
      assert.equal(response.status, expected, `${method} ${path}: ${JSON.stringify(data)}\n${logs}`);
      validateResponse(method, path, response.status, data);
      return data.data;
    }
    const loginBody = deviceId => ({ username: 'admin', password: 'password123', deviceId, platform: 'ios', deviceType: 'phone' });
    await api('GET', '/api/health');
    await api('GET', '/api/unknown', undefined, undefined, 404);
    await api('POST', '/api/health', {}, undefined, 404);
    await api('GET', '/api/messages/batch', undefined, undefined, 404);
    for (const route of protectedRoutes()) await api(route.method, route.path, undefined, undefined, 401);
    await api('GET', '/api/messages', undefined, undefined, 401);
    await api('GET', '/api/webhook/configs/0', undefined, undefined, 401);
    await api('PATCH', '/api/messages/not-a-uuid', {}, undefined, 401);
    await api('PATCH', '/api/webhook/configs/bad', {}, 'invalid', 401);
    await api('POST', '/api/auth/login', { ...loginBody('a'), password: 'wrong' }, undefined, 401);
    const login = await api('POST', '/api/auth/login', loginBody('a'));
    let token = login.accessToken;
    for (const [path, body, contentType, status, message] of [
      ['/api/webhook/configs/bad', '{', 'application/json', 400, 'invalid webhook config id'],
      ['/api/webhook/configs/0/enabled', '{}', 'text/plain', 400, 'invalid webhook config id'],
      ['/api/messages/not-a-uuid', '{', 'text/plain', 400, 'invalid message id'],
      ['/api/messages/not-a-uuid', 'x'.repeat(4 * 1024 * 1024 + 1), 'application/json', 413, 'request body too large'],
    ]) {
      const response = await fetch(origin + path, { method: 'PATCH', headers: { Authorization: `Bearer ${token}`, 'Content-Type': contentType }, body });
      const failure = await response.json();
      assert.equal(response.status, status, `${path}: ${JSON.stringify(failure)}`);
      assert.equal(failure.code, status);
      assert.equal(failure.msg, message);
      assert.equal(failure.data, null);
    }
    assert.equal(login.user.requiresAccountSetup, true);
    const signedGeneric = secret => ({ provider: 'generic', credentials: { type: 'hmacSha256', secret } });
    const anonymousGeneric = { provider: 'generic', credentials: { type: 'none' } };
    const config = await api('POST', '/api/webhook/configs/', { name: 'Monitor', platform: signedGeneric('test-webhook-secret') }, token);
    const configPath = `/api/webhook/configs/${config.id}`;
    assert.equal((await api('GET', configPath, undefined, token)).name, 'Monitor');
    assert.equal((await api('GET', `/api/webhook/configs/%2B${config.id}`, undefined, token)).id, config.id);
    await api('PATCH', configPath, { name: 'Updated' }, token);
    function localSql(command) {
      const result = spawnSync('wrangler', ['d1', 'execute', 'DB', '--local', '--persist-to', state, '--json', '--command', command], { encoding: 'utf8', env: { ...process.env, CI: 'true' } });
      assert.equal(result.status, 0, result.stdout + result.stderr);
      return JSON.parse(result.stdout)[0].results;
    }
    assert.equal(config.enabled, true);
    assert.match(config.key, /^[0-9a-f]{32}$/);
    assert.equal(config.secret, undefined);
    assert.equal(config.encryptedSecret, undefined);
    for (let i = 0; i < 6; i++) {
      await Promise.all([
        api('PATCH', configPath, { name: `Name ${i}` }, token),
        api('PATCH', configPath, { description: `Description ${i}` }, token),
      ]);
      const current = await api('GET', configPath, undefined, token);
      assert.equal(current.name, `Name ${i}`);
      assert.equal(current.description, `Description ${i}`);
      assert.equal(current.key, config.key);
       assert.deepEqual(current.platform, config.platform);
    }
    const beforeConfigNoop = localSql(`SELECT * FROM webhook_configs WHERE id=${config.id};`)[0];
    for (const field of ['name', 'platform', 'description', 'provider', 'source', 'secret']) {
      await api('PATCH', configPath, { [field]: null }, token, 400);
      await api('POST', '/api/webhook/configs', { name: 'Invalid', platform: anonymousGeneric, [field]: null }, token, 400);
    }
    for (const input of [{}, [], ['Name', 'generic'], { name: 'Missing' }, { provider: 'generic' }, { name: '', provider: 'generic' }, { name: 'Name', provider: '' }]) {
      await api('POST', '/api/webhook/configs', input, token, 400);
    }
    assert.equal(localSql('SELECT COUNT(*) AS count FROM webhook_configs;')[0].count, 1);
    for (const platform of [
      { provider: 'gitea', credentials: { secret: 'valid', unknown: true } },
      { provider: 'appstoreconnect', credentials: { secret: 'valid', privateKey: 'wrong-credential' } },
      { provider: 'github', credentials: { secret: 'valid', unknown: true } },
      { provider: 'github', credentials: ['positional-secret'] },
      ['github', { secret: 'positional-secret' }],
      { provider: 'generic', credentials: { type: 'none', secret: 'must-not-be-ignored' } },
      { provider: 'telegram', credentials: { secretToken: 'invalid:bot-token' } },
      { provider: 'gitlab', credentials: { type: 'signingToken', signingToken: 'whsec_!' } },
      { provider: 'stripe', credentials: { signingSecrets: [] } },
    ]) await api('POST', '/api/webhook/configs', { name: 'Rejected', platform }, token, 400);
    await api('PATCH', configPath, {}, token);
    await api('PATCH', configPath, { name: '', source: '', secret: '', provider: '', description: '' }, token, 400);
    await api('PATCH', configPath, { provider: 'unsupported' }, token, 400);
    assert.deepEqual(localSql(`SELECT * FROM webhook_configs WHERE id=${config.id};`)[0], beforeConfigNoop);
    assert.notEqual(beforeConfigNoop.encrypted_platform, 'test-webhook-secret');
    assert.equal(config.source, undefined);
    assert.equal(config.platform.credentials, undefined);
    // Signed deliveries below verify the freshly encrypted credential remains usable.
    await api('PATCH', configPath, { platform: signedGeneric('test-webhook-secret') }, token);
    const afterSecret = localSql(`SELECT * FROM webhook_configs WHERE id=${config.id};`)[0];
    assert.notEqual(afterSecret.encrypted_platform, beforeConfigNoop.encrypted_platform);
    assert.equal(afterSecret.key, beforeConfigNoop.key);
    assert.equal(afterSecret.created_at, beforeConfigNoop.created_at);
    await api('PATCH', configPath, { name: 'Updated' }, token);
    assert.equal((await api('GET', '/api/webhook/configs?page=9007199254740994&pageSize=1', undefined, token)).items.length, 0);
    await api('GET', '/api/webhook/configs?page=9223372036854775807&pageSize=100', undefined, token, 400);
    const disposable = await api('POST', '/api/webhook/configs', { name: 'Disposable', platform: anonymousGeneric }, token);
    const anonymousCiphertext = localSql(`SELECT encrypted_platform FROM webhook_configs WHERE id=${disposable.id};`)[0].encrypted_platform;
    const disposablePath = `/api/webhook/configs/${disposable.id}`;
    assert.equal((await api('PATCH', disposablePath + '/enabled', { enabled: false }, token)).enabled, false);
    assert.equal((await api('PATCH', disposablePath + '/enabled', { enabled: true }, token)).enabled, true);
    localSql(`UPDATE webhook_configs SET encrypted_platform='sensitive-error-probe' WHERE id=${disposable.id};`);
    // An internal credential decoding error stays generic in the real runtime,
    // including its logs; ordinary API error statuses retain their contract.
    await api('POST', disposable.url, { title: 'error probe' }, undefined, 500);
    assert.ok(!logs.includes('sensitive-error-probe'));
    await api('DELETE', disposablePath, undefined, token);
    const deletedConfig = localSql(`SELECT * FROM webhook_configs WHERE id=${disposable.id};`)[0];
    assert.equal(deletedConfig.deleted_at, deletedConfig.updated_at);
    await api('PATCH', disposablePath, { name: 'revived' }, token, 404);
    await api('PATCH', disposablePath + '/enabled', { enabled: true }, token, 404);
    await api('DELETE', disposablePath, undefined, token, 404);
    await api('GET', disposablePath, undefined, token, 404);
    assert.deepEqual(localSql(`SELECT * FROM webhook_configs WHERE id=${disposable.id};`)[0], deletedConfig);
    const streamResponse = await fetch(origin + '/api/messages/stream', { headers: { Authorization: `Bearer ${token}` } });
    assert.equal(streamResponse.status, 200, logs);
    const reader = streamResponse.body.getReader();
    const decoder = new TextDecoder();
    const readyEvent = decoder.decode((await reader.read()).value);
    assert.match(readyEvent, /event: ready/);
    validateSse(readyEvent);
    async function receive(payload, expected = 200) {
      const raw = JSON.stringify(payload);
      const signature = 'sha256=' + createHmac('sha256', 'test-webhook-secret').update(raw).digest('hex');
      const response = await fetch(origin + config.url, { method: 'POST', body: raw, headers: { 'X-NoMail-Signature': signature, 'Content-Type': 'application/json' } });
      const result = await response.json();
      assert.equal(response.status, expected, JSON.stringify(result) + logs);
      validateResponse('POST', config.url, response.status, result);
      return result.data;
    }
    const created = await receive({ id: 'event-1', title: 'Disk', content: 'hello\n 世界', level: 'urgent' });
    assert.equal(created.duplicate, false);
    assert.match(created.messageId, /^[0-9a-f]{8}-[0-9a-f]{4}-7/);
    const createdEvent = decoder.decode((await reader.read()).value);
    assert.match(createdEvent, /event: message.created/);
    validateSse(createdEvent);
    const createdDetail = await api('GET', `/api/messages/${created.messageId}`, undefined, token);
    assert.equal(createdEvent, `id: ${created.messageId}\nevent: message.created\ndata: ${JSON.stringify({ id: created.messageId, receivedAt: createdDetail.createdAt })}\n\n`);
    assert.equal(createdDetail.content, 'hello\n 世界');
    await reader.cancel();
    const duplicates = await Promise.all(Array.from({ length: 5 }, () => receive({ id: 'event-1', title: 'Disk' })));
    assert.ok(duplicates.every(item => item.duplicate && item.messageId === created.messageId));
    const list = await api('GET', '/api/messages?query=%E4%B8%96%E7%95%8C&status=unread&level=urgent&pinned=false', undefined, token);
    assert.equal(list.total, 1);
    assert.equal(list.items[0].summary, 'hello 世界');
    const normalizedList = await api('GET', '/api/messages?query=%20%E4%B8%96%E7%95%8C%20&status=%20unread%20&level=URGENT&pinned=F&page=0&pageSize=999', undefined, token);
    assert.equal(normalizedList.total, 1);
    assert.equal(normalizedList.page, 1);
    assert.equal(normalizedList.pageSize, 100);
    await testMessageCursors(api, localSql, token);
    const path = `/api/messages/${created.messageId}`;
    assert.equal((await api('PATCH', path, { status: 'read', pinned: true }, token)).pinned, true);
    assert.equal((await api('GET', '/api/messages/unread-count', undefined, token)).count, 0);
    assert.equal((await api('POST', '/api/messages/batch', { ids: [created.messageId, created.messageId], action: 'mark_unread' }, token)).affected, 1);
    assert.equal((await api('GET', path, undefined, token)).readAt, null);
    const raw = await fetch(origin + path + '/raw', { headers: { Authorization: `Bearer ${token}` } });
    assert.equal((await raw.json()).id, 'event-1');
    await api('GET', '/api/messages?from=bad', undefined, token, 400);
    await api('DELETE', path, undefined, token);
    assert.equal((await receive({ id: 'event-1', title: 'deleted' })).messageId, created.messageId);
    await receive({ title: 'no ID' });
    await receive({ title: 'no ID' });
    assert.equal((await api('GET', '/api/messages', undefined, token)).total, 2);
    const stateMessage = await receive({ id: 'state-test', title: 'State', content: 'body', level: 'high', extra: { keep: true } });
    const stateSecond = await receive({ id: 'state-second', title: 'Second' });
    const statePath = `/api/messages/${stateMessage.messageId}`;
    const stateSnapshot = await api('GET', statePath, undefined, token);
    for (let i = 0; i < 6; i++) {
      await api('PATCH', statePath, { status: 'unread', pinned: false }, token);
      await Promise.all([
        api('PATCH', statePath, { status: 'read' }, token),
        api('PATCH', statePath, { pinned: true }, token),
      ]);
      const current = await api('GET', statePath, undefined, token);
      assert.equal(current.status, 'read');
      assert.equal(current.pinned, true);
      assert.equal(typeof current.readAt, 'string');
      for (const field of ['title', 'content', 'source', 'level', 'provider', 'metadata', 'platformData', 'createdAt', 'expireAt']) assert.deepEqual(current[field], stateSnapshot[field]);
      const pinOnly = await api('PATCH', statePath, { pinned: false, level: 'urgent' }, token);
      assert.equal(pinOnly.readAt, current.readAt);
      assert.equal(pinOnly.level, 'high');
    }
    const boundaryIds = [stateMessage.messageId, ...Array.from({ length: 99 }, (_, i) => `00000000-0000-0000-0000-${(i + 1).toString(16).padStart(12, '0')}`)];
    assert.equal((await api('POST', '/api/messages/batch', { ids: boundaryIds, action: 'pin' }, token)).affected, 1);
    const beforeBadBatch = await api('GET', statePath, undefined, token);
    for (const body of [{ ids: [stateMessage.messageId, 'invalid'], action: 'unpin' }, { ids: Array(101).fill(stateMessage.messageId), action: 'unpin' }, { ids: [], action: 'pin' }, { ids: [stateMessage.messageId], action: 'invalid' }, { ids: null, action: 'pin' }, { ids: [stateMessage.messageId], action: null }]) await api('POST', '/api/messages/batch', body, token, 400);
    assert.deepEqual(await api('GET', statePath, undefined, token), beforeBadBatch);
    assert.equal((await api('GET', '/api/messages?page=9007199254740994&pageSize=1', undefined, token)).items.length, 0);
    // A second owner is seeded through local admin SQL, not a public signup API.
    // This uses the isolated test DB and never touches any remote/user database.
    const seed = spawnSync('wrangler', ['d1', 'execute', 'DB', '--local', '--persist-to', state, '--command', `INSERT INTO users(username,password_hash,created_at,updated_at) VALUES ('scope-other','not-used',strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now')); INSERT INTO webhook_configs(user_id,name,key,provider,encrypted_platform,enabled,created_at,updated_at) SELECT id,'Foreign','foreign-scope-key','generic','${anonymousCiphertext}',1,strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM users WHERE username='scope-other';`], { encoding: 'utf8', env: { ...process.env, CI: 'true' } });
    assert.equal(seed.status, 0, seed.stdout + seed.stderr);
    const foreignConfig = localSql("SELECT * FROM webhook_configs WHERE key='foreign-scope-key';")[0];
    const foreignConfigPath = `/api/webhook/configs/${foreignConfig.id}`;
    await api('GET', foreignConfigPath, undefined, token, 404);
    await api('PATCH', foreignConfigPath, { name: 'stolen' }, token, 404);
    await api('PATCH', foreignConfigPath + '/enabled', { enabled: false }, token, 404);
    await api('DELETE', foreignConfigPath, undefined, token, 404);
    assert.deepEqual(localSql("SELECT * FROM webhook_configs WHERE key='foreign-scope-key';")[0], foreignConfig);
    await api('PATCH', '/api/auth/account', { currentPassword: 'password123', username: 'scope-other' }, token, 409);
    assert.equal((await api('GET', '/api/auth/account', undefined, token)).username, 'admin');
    const foreign = await api('POST', '/api/webhook/foreign-scope-key', { id: 'foreign', title: 'Other owner' });
    const foreignPath = `/api/messages/${foreign.messageId}`;
    await api('GET', foreignPath, undefined, token, 404);
    await api('GET', foreignPath + '/raw', undefined, token, 404);
    await api('PATCH', foreignPath, { pinned: true }, token, 404);
    await api('DELETE', foreignPath, undefined, token, 404);
    const scopedBatch = await api('POST', '/api/messages/batch', { ids: [stateMessage.messageId, stateMessage.messageId, stateSecond.messageId, foreign.messageId, created.messageId], action: 'mark_read' }, token);
    assert.equal(scopedBatch.affected, 2);
    assert.equal((await api('GET', statePath, undefined, token)).readAt, (await api('GET', `/api/messages/${stateSecond.messageId}`, undefined, token)).readAt);
    const scopeCheck = spawnSync('wrangler', ['d1', 'execute', 'DB', '--local', '--persist-to', state, '--json', '--command', `SELECT status,pinned FROM notify_messages WHERE id='${foreign.messageId}';`], { encoding: 'utf8', env: { ...process.env, CI: 'true' } });
    assert.equal(scopeCheck.status, 0, scopeCheck.stdout + scopeCheck.stderr);
    const foreignRow = JSON.parse(scopeCheck.stdout)[0].results[0];
    assert.equal(foreignRow.status, 'unread');
    assert.equal(foreignRow.pinned, 0);
    for (const [body, contentType] of [['{', 'application/json'], ['{}', 'text/plain']]) {
      const response = await fetch(origin + configPath, { method: 'PATCH', headers: { Authorization: `Bearer ${token}`, 'Content-Type': contentType }, body });
      assert.equal(response.status, 400);
      validateResponse('PATCH', configPath, response.status, await response.json());
    }
    const plain = ' plain text\n 世界 \n';
    const plainResponse = await fetch(origin + config.url, { method: 'POST', body: plain, headers: { 'Content-Type': 'text/plain', 'X-NoMail-Title': 'Plain', 'X-NoMail-Level': 'high', 'X-NoMail-Signature': 'sha256=' + createHmac('sha256', 'test-webhook-secret').update(plain).digest('hex') } });
    const plainAck = await plainResponse.json();
    assert.equal(plainResponse.status, 200);
    validateResponse('POST', config.url, 200, plainAck);
    const plainRaw = await fetch(origin + `/api/messages/${plainAck.data.messageId}/raw`, { headers: { Authorization: `Bearer ${token}` } });
    assert.match(plainRaw.headers.get('Content-Type'), /text\/plain/);
    assert.equal(await plainRaw.text(), plain);
    for (const level of ['low', 'normal', 'high', 'urgent']) {
      const received = await receive({ id: `priority-${level}`, title: `Priority ${level}`, level });
      const detail = await api('GET', `/api/messages/${received.messageId}`, undefined, token);
      assert.equal(detail.level, level);
      assert.equal(Date.parse(detail.expireAt) - Date.parse(detail.createdAt), 3 * 86_400_000);
      const filtered = await api('GET', `/api/messages?level=${level}`, undefined, token);
      assert.ok(filtered.items.some(item => item.id === received.messageId));
      assert.ok(filtered.items.every(item => item.level === level));
    }
    for (const oldLevel of ['success', 'warning', 'critical', 'error']) {
      await api('GET', `/api/messages?level=${oldLevel}`, undefined, token, 400);
    }
    // A message keeps common fields, structured platform details and raw content
    // together, while NoMail state changes independently.
    const github = await api('POST', '/api/webhook/configs', { name: 'GitHub', platform: { provider: 'github', credentials: { secret: 'github-secret' } } }, token);
    async function githubReceive(payload, event, delivery) {
      const raw = JSON.stringify(payload);
      const signature = 'sha256=' + createHmac('sha256', 'github-secret').update(raw).digest('hex');
      const response = await fetch(origin + github.url, { method: 'POST', body: raw, headers: { 'Content-Type': 'application/json', 'X-GitHub-Event': event, 'X-GitHub-Delivery': delivery, 'X-Hub-Signature-256': signature } });
      const result = await response.json();
      assert.equal(response.status, 200, JSON.stringify(result) + logs);
      validateResponse('POST', github.url, response.status, result);
      return result.data;
    }
    const comment = { action: 'created', repository: { id: 1, full_name: 'owner/repo', private: true, created_at: '2026-01-01T00:00:00Z' },
      issue: { id: 2, number: 123, title: 'Login failure', body: 'Detailed issue', labels: [{ id: 3, name: 'bug', color: 'ff0000' }], assignees: [{ id: 4, login: 'bob', type: 'User' }], pull_request: { html_url: 'https://example.com/pull/123' } },
      comment: { id: 456, body: 'Reply text', user: { id: 4, login: 'bob' }, created_at: '2000-01-01T00:00:00Z', updated_at: '2000-01-02T00:00:00Z' },
      custom: { not_in_summary: true } };
    const longId = 'x'.repeat(300) + 'a';
    const reply = await githubReceive(comment, 'issue_comment', longId);
    const otherReply = await githubReceive(comment, 'issue_comment', 'x'.repeat(300) + 'b');
    assert.equal(reply.duplicate, false);
    assert.equal(otherReply.duplicate, false);
    assert.notEqual(reply.messageId, otherReply.messageId);
    assert.equal((await githubReceive(comment, 'issue_comment', longId)).messageId, reply.messageId);
    const replyPath = `/api/messages/${reply.messageId}`;
    const before = await api('GET', replyPath, undefined, token);
    assert.equal(before.level, 'normal');
    assert.equal(before.provider, 'github');
    assert.equal(before.metadata.eventType, 'issue_comment');
    assert.equal(before.metadata.deduplicationKey.value, longId);
    assert.equal(before.platformData.event.issue.number, 123);
    assert.equal(before.platformData.event.issue.id, 2);
    assert.equal(before.platformData.event.issue.assignees[0].type, 'User');
    assert.equal(before.platformData.event.issue.pull_request.html_url, 'https://example.com/pull/123');
    assert.equal(before.platformData.event.repository.private, true);
    assert.equal(before.platformData.event.repository.created_at, '2026-01-01T00:00:00Z');
    assert.equal(before.platformData.event.comment.updated_at, '2000-01-02T00:00:00Z');
    assert.equal(before.platformData.event.issue.labels[0].name, 'bug');
    assert.equal(before.platformData.event.comment.user.login, 'bob');
    assert.notEqual(before.createdAt, comment.comment.created_at);
    const after = await api('PATCH', replyPath, { status: 'read', pinned: true }, token);
    assert.deepEqual(after.platformData, before.platformData);
    assert.deepEqual(after.metadata, before.metadata);
    assert.equal(after.createdAt, before.createdAt);
    assert.equal(after.pinned, true);
    const original = await fetch(origin + replyPath + '/raw', { headers: { Authorization: `Bearer ${token}` } });
    assert.deepEqual(await original.json(), comment);
    const build = await githubReceive({ action: 'completed', workflow_run: { id: 10, name: 'CI', status: 'completed', conclusion: 'failure', head_branch: 'main', run_number: 11, display_title: 'Build PR', run_attempt: 2, actor: { id: 4, login: 'bob' } } }, 'workflow_run', 'ci-failure');
    const buildDetail = await api('GET', `/api/messages/${build.messageId}`, undefined, token);
    assert.equal(buildDetail.level, 'normal');
    assert.equal(buildDetail.platformData.event.workflow_run.run_attempt, 2);
    assert.equal(buildDetail.platformData.event.workflow_run.actor.id, 4);
    assert.equal(buildDetail.platformData.event.workflow_run.conclusion, 'failure');
    assert.equal(buildDetail.platformData.event.workflow_run.head_branch, 'main');
    assert.equal(Date.parse(buildDetail.expireAt) - Date.parse(buildDetail.createdAt), 3 * 86_400_000);
    await api('PATCH', `/api/webhook/configs/${github.id}`, { platform: signedGeneric('github-secret') }, token);
    const genericRaw = JSON.stringify({ id: longId, title: 'same value, different scope' });
    const genericResult = await fetch(origin + github.url, { method: 'POST', body: genericRaw, headers: { 'Content-Type': 'application/json', 'X-NoMail-Signature': 'sha256=' + createHmac('sha256', 'github-secret').update(genericRaw).digest('hex') } });
    const scoped = (await genericResult.json()).data;
    assert.equal(genericResult.status, 200);
    assert.equal(scoped.duplicate, false);
    const historical = await api('GET', replyPath, undefined, token);
    assert.equal(historical.provider, 'github');
    assert.deepEqual(historical.platformData, before.platformData);
    const feishu = await api('POST', '/api/webhook/configs', { name: 'Feishu', platform: { provider: 'feishu', credentials: { verificationToken: 'feishu-token' } } }, token);
    const feishuPayload = { schema: '2.0', header: { token: 'feishu-token', event_id: 'f-1', event_type: 'im.message.receive_v1' }, event: { sender: { sender_id: { open_id: 'user-open-id' }, sender_type: 'user' }, message: { message_id: 'm-1', message_type: 'text', chat_id: 'chat-1', content: JSON.stringify({ text: 'hello', unknown: true }) } } };
    const feishuMessage = await api('POST', feishu.url, feishuPayload);
    const feishuDetail = await api('GET', `/api/messages/${feishuMessage.messageId}`, undefined, token);
    assert.equal(feishuDetail.metadata.version, '2.0');
    assert.equal(feishuDetail.platformData.event.V2.message.content_json.text, 'hello');
    assert.equal(feishuDetail.platformData.event.V2.message.content_json.unknown, true);
    assert.equal(feishuDetail.platformData.event.V2.sender.open_id, 'user-open-id');
    // Both credentials must remain independent through D1 storage/decryption.
    const encryptedFeishu = await api('POST', '/api/webhook/configs', { name: 'Encrypted Feishu', platform: { provider: 'feishu', credentials: { verificationToken: 'verification-token', encryptKey: 'different-encryption-key' } } }, token);
    const iv = Buffer.alloc(16, 7);
    const cipher = createCipheriv('aes-256-cbc', createHash('sha256').update('different-encryption-key').digest(), iv);
    const ciphertext = Buffer.concat([iv, cipher.update(JSON.stringify({ type: 'url_verification', token: 'verification-token', challenge: 'test-challenge' })), cipher.final()]);
    const challenge = await fetch(origin + encryptedFeishu.url, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ encrypt: ciphertext.toString('base64') }) });
    assert.equal(challenge.status, 200);
    assert.deepEqual(await challenge.json(), { challenge: 'test-challenge' });
    assert.equal(encryptedFeishu.platform.authentication, 'feishuEncrypted');
    for (const platform of [
      { provider: 'gitlab', credentials: { type: 'signingToken', signingToken: 'whsec_cGxhdGZvcm0tc2VjcmV0' } },
      { provider: 'gitea', credentials: { secret: 'platform-secret' } },
      { provider: 'appstoreconnect', credentials: { secret: 'platform-secret' } },
      { provider: 'gitlab', credentials: { type: 'secretToken', secretToken: 'platform-secret' } },
      { provider: 'stripe', credentials: { signingSecrets: ['whsec_retired', 'whsec_platform-secret'] } },
      { provider: 'slack', credentials: { signingSecret: 'platform-secret' } },
      { provider: 'telegram', credentials: { secretToken: 'platform-secret' } },
      { provider: 'shopify', credentials: { clientSecret: 'platform-secret' } },
    ]) {
      const item = await api('POST', '/api/webhook/configs', { name: 'Platform test', platform }, token);
      const path = `/api/webhook/configs/${item.id}`;
      assert.equal(item.platform.provider, platform.provider);
      assert.equal(item.platform.credentials, undefined);
      assert.ok(!JSON.stringify(item).includes('platform-secret'));
      assert.deepEqual(await api('GET', path, undefined, token), item);
      const payloads = {
        gitea: { action: 'assigned', issue: { number: 1, title: 'Test', is_locked: true }, sender: { id: -2, login: 'actions' } },
        appstoreconnect: { data: { type: 'appStoreVersionAppVersionStateUpdated', id: 'matrix', version: 1, attributes: { oldValue: 'PREPARE_FOR_SUBMISSION', newValue: 'READY_FOR_REVIEW' }, relationships: { instance: { data: { type: 'appStoreVersions', id: 'example-version' } } } }, future: { keep: true } },
        gitlab: { object_kind: 'issue', object_attributes: { title: 'Test' } },
        stripe: { id: 'evt_matrix', object: 'event', type: 'payment_intent.succeeded', data: { object: { id: 'pi_test' } } },
        slack: { type: 'event_callback', event_id: 'EvMatrix', event: { type: 'message', text: 'Test' } },
        telegram: { update_id: 123, message: { message_id: 1, chat: { id: 1, type: 'private' }, text: 'Test' } },
        shopify: { id: 123, title: 'Test' },
      };
      const raw = JSON.stringify(payloads[platform.provider]);
      const time = Math.floor(Date.now() / 1000).toString();
      const sign = (key, input, encoding = 'hex') => createHmac('sha256', key).update(input).digest(encoding);
      let headers;
      switch (platform.provider) {
        case 'gitea': headers = { 'X-Gitea-Event': 'issues', 'X-Gitea-Event-Type': 'issue_assign', 'X-Gitea-Delivery': 'matrix', 'X-Gitea-Signature': sign('platform-secret', raw) }; break;
        case 'appstoreconnect': headers = { 'X-Apple-Signature': 'hmacsha256=' + sign('platform-secret', raw) }; break;
        case 'gitlab':
          headers = platform.credentials.type === 'signingToken'
            ? { 'X-Gitlab-Event': 'Issue Hook', 'webhook-id': 'matrix', 'webhook-timestamp': time, 'webhook-signature': 'v1,' + sign('platform-secret', `matrix.${time}.${raw}`, 'base64') }
            : { 'X-Gitlab-Event': 'Issue Hook', 'X-Gitlab-Token': 'platform-secret' };
          break;
        case 'stripe': headers = { 'Stripe-Signature': `t=${time},v1=${sign('whsec_platform-secret', `${time}.${raw}`)}` }; break;
        case 'slack': headers = { 'X-Slack-Request-Timestamp': time, 'X-Slack-Signature': 'v0=' + sign('platform-secret', `v0:${time}:${raw}`) }; break;
        case 'telegram': headers = { 'X-Telegram-Bot-Api-Secret-Token': 'platform-secret' }; break;
        case 'shopify': headers = { 'X-Shopify-Topic': 'products/create', 'X-Shopify-Shop-Domain': 'test.myshopify.com', 'X-Shopify-API-Version': '2026-10', 'X-Shopify-Hmac-Sha256': sign('platform-secret', raw, 'base64') }; break;
      }
      const response = await fetch(origin + item.url, { method: 'POST', body: raw, headers: { 'Content-Type': 'application/json', ...headers } });
      const ack = await response.json();
      assert.equal(response.status, 200, JSON.stringify(ack));
      validateResponse('POST', item.url, 200, ack);
      assert.equal((await api('GET', `/api/messages/${ack.data.messageId}`, undefined, token)).provider, platform.provider);
      if (platform.provider === 'telegram') {
        const fixtures = JSON.parse(await readFile(new URL('../../nomail-core/src/webhook/platforms/telegram/fixtures/updates.json', import.meta.url), 'utf8'));
        for (const fixture of fixtures) {
          const payload = { ...fixture, future_metadata: { retained: true } };
          const response = await fetch(origin + item.url, { method: 'POST', body: JSON.stringify(payload), headers: { 'Content-Type': 'application/json', ...headers } });
          const ack = await response.json();
          assert.equal(response.status, 200, JSON.stringify(ack));
          validateResponse('POST', item.url, 200, ack);
          assert.equal(ack.data.duplicate, false);
          const path = `/api/messages/${ack.data.messageId}`;
          const detail = await api('GET', path, undefined, token);
          assert.deepEqual(detail.platformData.event, fixture);
          assert.equal(detail.metadata.deduplicationKey.scope, 'telegram:bot-update');
          const original = await fetch(origin + path + '/raw', { headers: { Authorization: `Bearer ${token}` } });
          assert.equal(original.status, 200);
          assert.deepEqual(await original.json(), payload);
          const replay = await fetch(origin + item.url, { method: 'POST', body: JSON.stringify(fixture), headers: { 'Content-Type': 'application/json', ...headers } });
          const duplicate = await replay.json();
          assert.equal(replay.status, 200, JSON.stringify(duplicate));
          validateResponse('POST', item.url, 200, duplicate);
          assert.equal(duplicate.data.duplicate, true);
          assert.equal(duplicate.data.messageId, ack.data.messageId);
        }
      }
      if (platform.provider === 'gitea') {
        const detail = await api('GET', `/api/messages/${ack.data.messageId}`, undefined, token);
        assert.equal(detail.platformData.provider, 'gitea');
        assert.equal(detail.platformData.event.sender.id, -2);
        assert.equal(detail.platformData.event.issue.is_locked, true);
        assert.equal(detail.platformData.event.event_type, 'issue_assign');
        assert.equal(detail.metadata.deduplicationKey.scope, 'gitea:delivery');
        for (const [delivery, duplicate] of [['matrix', true], ['matrix-replay', false]]) {
          const response = await fetch(origin + item.url, { method: 'POST', body: new URLSearchParams({ payload: raw }).toString(), headers: { ...headers, 'X-Gitea-Delivery': delivery, 'Content-Type': 'application/x-www-form-urlencoded' } });
          const replay = await response.json();
          assert.equal(response.status, 200, JSON.stringify(replay));
          validateResponse('POST', item.url, 200, replay);
          assert.equal(replay.data.duplicate, duplicate);
          assert.equal(replay.data.messageId === ack.data.messageId, duplicate);
        }
        const original = await fetch(origin + `/api/messages/${ack.data.messageId}/raw`, { headers: { Authorization: `Bearer ${token}` } });
        assert.deepEqual(await original.json(), payloads.gitea);
      }
      await api('POST', item.url, { title: 'unauthenticated' }, undefined, 401);
      if (platform.provider === 'appstoreconnect') {
        const detail = await api('GET', `/api/messages/${ack.data.messageId}`, undefined, token);
        assert.equal(detail.platformData.provider, 'appstoreconnect');
        assert.equal(detail.platformData.event.data.type, 'appStoreVersionAppVersionStateUpdated');
        assert.equal(detail.platformData.event.data.attributes.oldValue, 'PREPARE_FOR_SUBMISSION');
        assert.equal(detail.platformData.event.data.attributes.newValue, 'READY_FOR_REVIEW');
        assert.equal(detail.platformData.event.data.relationships.instance.data.id, 'example-version');
        assert.equal(detail.metadata.version, '1');
        assert.equal(detail.metadata.deduplicationKey.scope, 'appstoreconnect:event');
        const next = structuredClone(payloads.appstoreconnect);
        next.data.id = 'matrix-next-event';
        for (const [body, duplicate] of [[raw, true], [JSON.stringify(next), false]]) {
          const response = await fetch(origin + item.url, { method: 'POST', body, headers: { 'Content-Type': 'application/json', 'X-Apple-Signature': 'hmacsha256=' + sign('platform-secret', body) } });
          const replay = await response.json();
          assert.equal(response.status, 200, JSON.stringify(replay));
          validateResponse('POST', item.url, 200, replay);
          assert.equal(replay.data.duplicate, duplicate);
          assert.equal(replay.data.messageId === ack.data.messageId, duplicate);
        }
        const original = await fetch(origin + `/api/messages/${ack.data.messageId}/raw`, { headers: { Authorization: `Bearer ${token}` } });
        assert.deepEqual(await original.json(), payloads.appstoreconnect);
      }
      await api('PATCH', path, { platform: { provider: 'github' } }, token, 400);
      assert.deepEqual(await api('GET', path, undefined, token), item);
      const switched = await api('PATCH', path, { platform: anonymousGeneric }, token);
      assert.equal(switched.platform.authentication, 'none');
      assert.equal(switched.key, item.key);
      await api('POST', item.url, { title: 'explicit anonymous' });
      await api('DELETE', path, undefined, token);
    }
    for (const item of (await api('GET', '/api/messages', undefined, token)).items) {
      assert.equal(typeof item.level, 'string');
      assert.equal(Object.hasOwn(item, 'platformData'), false);
      assert.equal(Object.hasOwn(item, 'raw_payload'), false);
    }
    const refreshes = await Promise.all(Array.from({ length: 2 }, () => fetch(origin + '/api/auth/refresh-token', { method: 'POST', headers: { 'Content-Type': 'application/json', 'User-Agent': 'nomail-refresh-agent' }, body: JSON.stringify({ refreshToken: login.refreshToken }) })));
    assert.deepEqual(refreshes.map(r => r.status).sort(), [200, 401]);
    const refreshed = (await refreshes.find(r => r.status === 200).json()).data;
    token = refreshed.accessToken;
    // Deployment-specific capacity errors still obey the public Core error
    // envelope, rather than leaking a private DO's plain-text response.
    const subscriptions = [];
    let limited = false;
    try {
      for (let i = 0; i < 34; i++) {
        const response = await fetch(origin + '/api/messages/stream', { headers: { Authorization: `Bearer ${token}` } });
        if (response.status === 429) {
          validateResponse('GET', '/api/messages/stream', 429, await response.json());
          limited = true;
          break;
        }
        assert.equal(response.status, 200);
        subscriptions.push(response);
      }
      assert.equal(limited, true);
    } finally {
      await Promise.all(subscriptions.map(response => response.body.cancel()));
    }
    const devices = await api('GET', '/api/auth/devices', undefined, token);
    assert.equal(Object.hasOwn(devices[0], 'pushEnabled'), false);
    assert.equal(devices.find(device => device.deviceId === 'a').userAgent, 'nomail-refresh-agent');
    const rejectedRefresh = await fetch(origin + '/api/auth/refresh-token', { method: 'POST', headers: { 'Content-Type': 'application/json', 'User-Agent': 'must-not-update-device' }, body: JSON.stringify({ refreshToken: login.refreshToken }) });
    assert.equal(rejectedRefresh.status, 401);
    assert.equal((await api('GET', '/api/auth/devices', undefined, token)).find(device => device.deviceId === 'a').userAgent, 'nomail-refresh-agent');
    // D1 batch transactions serialize device-limit decisions across requests.
    const extraLogins = await Promise.all(['b', 'c', 'd', 'e'].map(id => api('POST', '/api/auth/login', loginBody(id))));
    const newest = await api('POST', '/api/auth/login', loginBody('last'));
    token = newest.accessToken;
    const activeDevices = await api('GET', '/api/auth/devices', undefined, token);
    assert.equal(activeDevices.filter(device => device.loggedIn).length, 3);
    const accessStatuses = await Promise.all(extraLogins.map(login => fetch(origin + '/api/auth/account', { headers: { Authorization: `Bearer ${login.accessToken}` } }).then(r => r.status)));
    assert.equal(accessStatuses.filter(status => status === 200).length, 2);
    await api('POST', '/api/push/devices', {}, token, 404);
    await api('PATCH', '/api/push/devices/a', {}, token, 404);
    await api('DELETE', '/api/push/devices/a', undefined, token, 404);
    await api('PATCH', '/api/auth/account', { currentPassword: 'password123', newPassword: 'newpassword123' }, token);
    await api('GET', '/api/messages', undefined, token, 401);
    const again = await api('POST', '/api/auth/login', { ...loginBody('a'), password: 'newpassword123' });
    token = again.accessToken;
    await api('DELETE', '/api/auth/devices/a/session', undefined, token);
    await api('GET', '/api/messages', undefined, token, 401);
  } finally {
    if (child?.exitCode === null) { child.kill('SIGTERM'); await stopped; }
    await rm(state, { recursive: true, force: true });
  }
});
