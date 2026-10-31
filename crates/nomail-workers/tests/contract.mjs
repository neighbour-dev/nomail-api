// Test helper only. The route/schema source is the generated Core OpenAPI, not a
// second hand-written HTTP interface in JavaScript.
import Ajv2020 from 'ajv/dist/2020.js';
import addFormats from 'ajv-formats';
import { readFileSync } from 'node:fs';
import assert from 'node:assert/strict';

const document = JSON.parse(readFileSync(new URL('../../../docs/swagger.json', import.meta.url), 'utf8'));
const id = 'https://nomail.invalid/openapi.json';
const ajv = new Ajv2020({ strict: false, allErrors: true, validateFormats: true, logger: false });
addFormats(ajv);
ajv.addSchema(document, id);
const validators = new Map();
const escape = value => value.replaceAll('~', '~0').replaceAll('/', '~1');

export function validateResponse(method, path, status, value) {
  if (status >= 400) {
    assert.equal(value.code, status);
    assert.equal(value.data, null);
  }
  const segments = new URL(path, 'http://local').pathname.replace(/\/+$/, '').split('/');
  const matches = Object.entries(document.paths).filter(([template]) => {
    const expected = template.split('/');
    return expected.length === segments.length && expected.every((segment, index) => segment.startsWith('{') || segment === segments[index]);
  }).sort(([a], [b]) => b.split('/').filter(s => !s.startsWith('{')).length - a.split('/').filter(s => !s.startsWith('{')).length);
  const template = matches[0]?.[0];
  const operation = template && document.paths[template][method.toLowerCase()];
  const json = operation?.responses[status]?.content?.['application/json'];
  if (status === 200 && (json?.schema?.properties?.code || (json?.schema?.oneOf && value && Object.hasOwn(value, 'code')))) {
    assert.equal(value.code, 0);
    assert.equal(value.msg, 'success');
  }
  const pointer = json ? `/paths/${escape(template)}/${method.toLowerCase()}/responses/${status}/content/application~1json/schema`
    : '/paths/~1api~1health/get/responses/500/content/application~1json/schema';
  const key = pointer;
  if (!validators.has(key)) validators.set(key, ajv.compile({ $ref: id + '#' + pointer }));
  const validator = validators.get(key);
  assert.ok(validator(value), `${method} ${path} HTTP ${status}: response violates Core contract\n${ajv.errorsText(validator.errors)}\n${JSON.stringify(value)}`);
}

export function protectedRoutes() {
  return Object.entries(document.paths).flatMap(([path, methods]) => Object.entries(methods)
    .filter(([, operation]) => operation.security?.some(s => s.BearerAuth))
    .map(([method, operation]) => {
      let sample = path;
      for (const parameter of operation.parameters ?? []) {
        if (parameter.in !== 'path') continue;
        const value = parameter.schema.type === 'integer' ? '1' : parameter.schema.format === 'uuid' ? '0198c000-0000-7000-8000-000000000001' : 'phone';
        sample = sample.replace(`{${parameter.name}}`, value);
      }
      return { method: method.toUpperCase(), path: sample };
    }));
}

export function validateSse(text) {
  const source = Object.entries(document.paths).flatMap(([path, methods]) => Object.entries(methods)
    .filter(([, operation]) => operation['x-sse-events'])
    .map(([method]) => `/paths/${escape(path)}/${method}/x-sse-events`))[0];
  assert.ok(source, 'Core contract must declare SSE event schemas');
  if (!validators.has(source)) validators.set(source, ajv.compile({ $ref: id + '#' + source }));
  const validator = validators.get(source);
  for (const block of text.split('\n\n').filter(Boolean)) {
    const lines = block.split('\n');
    const name = lines.find(line => line.startsWith('event: '))?.slice(7);
    if (!name) continue; // heartbeat comment
    const data = JSON.parse(lines.filter(line => line.startsWith('data: ')).map(line => line.slice(6)).join('\n'));
    const logical = { event: name, data };
    assert.ok(validator(logical), `SSE violates Core contract: ${ajv.errorsText(validator.errors)}`);
    const eventId = lines.find(line => line.startsWith('id: '))?.slice(4);
    if (eventId) assert.equal(eventId, data.id);
  }
}
