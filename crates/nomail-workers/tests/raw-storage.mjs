import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

// Schema-only upgrade test in a separate local D1 instance. No legacy payload
// transformation and no deployment/user database is accessed by this helper.
export async function testRawStorageUpgrade(root) {
  const state = join(root, 'raw-storage-upgrade');
  const migrations = join(state, 'old-migrations');
  await mkdir(migrations, { recursive: true });
  for (const file of ['0001_baseline.sql', '0002_message_cursor.sql']) {
    await copyFile(new URL('../migrations/' + file, import.meta.url), join(migrations, file));
  }
  const config = join(state, 'old.toml');
  const original = await readFile(new URL('../wrangler.toml', import.meta.url), 'utf8');
  await writeFile(config, original.replace('migrations_dir = "migrations"', `migrations_dir = ${JSON.stringify(migrations)}`));
  const run = (args, succeeds = true) => {
    const result = spawnSync('wrangler', args, { encoding: 'utf8', env: { ...process.env, CI: 'true' } });
    if (succeeds) assert.equal(result.status, 0, result.stdout + result.stderr);
    else assert.notEqual(result.status, 0, 'upgrade must reject nonempty messages');
    return result;
  };
  const sql = command => JSON.parse(run(['d1', 'execute', 'DB', '--local', '--persist-to', state, '--json', '--command', command]).stdout)[0].results;
  run(['d1', 'migrations', 'apply', 'DB', '--config', config, '--local', '--persist-to', state]);
  sql(`INSERT INTO webhook_configs(id,created_at,updated_at,user_id,name,key,provider,encrypted_platform) VALUES (1,'2026','2026',1,'Kept config','kept-key','generic','ciphertext-kept'); INSERT INTO notify_messages(id,created_at,updated_at,webhook_config_id,provider,event_type,platform_data,title,content,level,raw_payload,content_type) VALUES ('10000000-0000-0000-0000-000000000001','2026','2026',1,'generic','message','{"provider":"generic"}','keep me','body','normal','{}','application/json');`);
  const rejected = run(['d1', 'migrations', 'apply', 'DB', '--local', '--persist-to', state], false);
  assert.match(rejected.stdout + rejected.stderr, /clear_notify_messages_before_raw_storage/);
  let columns = sql('PRAGMA table_info(notify_messages);').map(c => c.name);
  assert.ok(columns.includes('platform_data'));
  assert.ok(!columns.includes('raw_headers'));
  assert.equal(sql('SELECT title FROM notify_messages;')[0].title, 'keep me');
  assert.equal(sql("SELECT name FROM sqlite_master WHERE name='nomail_raw_storage_upgrade_guard';").length, 0);
  // Simulate the operator clearing ONLY messages before retrying the upgrade.
  sql('DELETE FROM notify_messages;');
  run(['d1', 'migrations', 'apply', 'DB', '--local', '--persist-to', state]);
  run(['d1', 'migrations', 'apply', 'DB', '--local', '--persist-to', state]);
  columns = sql('PRAGMA table_info(notify_messages);').map(c => c.name);
  assert.ok(columns.includes('raw_headers'));
  assert.ok(!columns.includes('platform_data'));
  assert.equal(sql('SELECT encrypted_platform FROM webhook_configs WHERE id=1;')[0].encrypted_platform, 'ciphertext-kept');
}
