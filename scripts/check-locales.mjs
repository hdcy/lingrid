import { readdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve('src/i18n/locales');
const files = readdirSync(root).filter(file => file.endsWith('.json'));
const locales = new Set();
function flatten(object, prefix = '', result = {}) {
  if (!object || typeof object !== 'object' || Array.isArray(object)) throw new Error(`${prefix}: expected an object`);
  for (const [key, value] of Object.entries(object)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (typeof value === 'string' && value.trim()) result[path] = value;
    else flatten(value, path, result);
  }
  return result;
}
const baseline = flatten(JSON.parse(readFileSync(resolve(root, 'en.json'), 'utf8')).messages);
const parameters = text => [...text.matchAll(/\{([^{}]+)\}/g)].map(match => match[1]).sort().join(',');
for (const file of files) {
  try {
    const { meta, messages } = JSON.parse(readFileSync(resolve(root, file), 'utf8'));
    if (!meta || typeof meta.locale !== 'string' || !meta.locale.trim()
        || typeof meta.nativeName !== 'string' || !meta.nativeName.trim()
        || !['ltr', 'rtl'].includes(meta.direction)) throw new Error('invalid language metadata');
    new Intl.Locale(meta.locale);
    const canonical = Intl.getCanonicalLocales(meta.locale)[0];
    if (locales.has(canonical)) throw new Error(`duplicate locale: ${meta.locale}`);
    locales.add(canonical);
    const actual = flatten(messages);
    for (const key of new Set([...Object.keys(baseline), ...Object.keys(actual)])) {
      if (!(key in baseline) || !(key in actual)) throw new Error(`translation key mismatch: ${key}`);
      if (parameters(baseline[key]) !== parameters(actual[key])) throw new Error(`parameter mismatch: ${key}`);
    }
  } catch (error) { throw new Error(`${file}: ${error.message}`, { cause: error }); }
}
console.log(`Validated ${files.length} language files (${Object.keys(baseline).length} keys each).`);
