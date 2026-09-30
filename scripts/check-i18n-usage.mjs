#!/usr/bin/env node
//
// Fails when the source asks for a translation key the catalogs do not define.
//
// check-i18n.sh answers "is anything still hardcoded?"; this answers the mirror
// question - "does every t() call actually resolve?". A missing key is not a
// compile error, it silently renders the raw key path in the UI, which is how
// `core.pipeline.resetDone` once reached a toast.
//
// It also checks that a call's params match the placeholders of the entry it
// asks for. A name that does not match renders a literal `{name}`, and it does
// so in only one language, so no other check sees it.
//
// Only statically written keys are checked. Keys built at runtime (`t('a.' +
// group)`, `` t(`a.${x}`) ``) cannot be resolved here; those rely on the `te()`
// guards at their call sites.
//
// Usage: scripts/check-i18n-usage.mjs [src-dir]   (default: <repo>/frontend/src)

import { readdirSync, readFileSync, statSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT = resolve(SCRIPT_DIR, '..')
const SRC_DIR = resolve(process.argv[2] ?? join(REPO_ROOT, 'frontend/src'))
const LOCALES_DIR = join(SRC_DIR, 'i18n/locales')

/** Dotted paths for every leaf in a nested catalog object. */
function flatten(node, prefix = '', out = new Map()) {
  for (const [key, value] of Object.entries(node)) {
    const path = prefix ? `${prefix}.${key}` : key
    if (value && typeof value === 'object') flatten(value, path, out)
    else out.set(path, value)
  }
  return out
}

/**
 * The names a `t()` call's params object binds.
 *
 * `{ a: b, c: d() }` and the shorthand `{ a, c }` both name a param; a spread
 * or a computed key does not, and is not counted either way.
 */
function passedNames(body) {
  return body
    .split(',')
    .map((part) => part.split(':')[0].trim())
    .filter((name) => /^\w+$/.test(name))
}

function readCatalog(locale) {
  const dir = join(LOCALES_DIR, locale)
  const merged = new Map()
  for (const file of readdirSync(dir)) {
    if (!file.endsWith('.json')) continue
    for (const [key, value] of flatten(JSON.parse(readFileSync(join(dir, file), 'utf8')))) {
      merged.set(key, value)
    }
  }
  return merged
}

function sourceFiles(dir, acc = []) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry)
    if (statSync(path).isDirectory()) {
      // The catalogs themselves are the definition, not a caller.
      if (entry !== 'i18n') sourceFiles(path, acc)
    } else if (/\.(vue|ts)$/.test(entry)) {
      acc.push(path)
    }
  }
  return acc
}

// `t('a.b')`, `i18n.global.t('a.b')` — the leading quote is what keeps `te('…')`,
// `t(variable)` and template-literal keys out of the match.
const CALL = /(?:\bt|\.t)\(\s*'([^']*)'/g
// The same call with its params object, when one follows it. Nested braces stop
// the capture, which leaves that call's params unchecked rather than misread.
const CALL_WITH_PARAMS = /(?:\bt|\.t)\(\s*'([^']*)'(?:\s*,\s*\{([^{}]*)\})?/g
const STATIC_KEY = /^[a-z][A-Za-z0-9]*(?:\.[A-Za-z0-9_]+)+$/
const PLACEHOLDER = /\{(\w+)\}/g

if (!statSync(LOCALES_DIR).isDirectory()) {
  console.error(`::error::${LOCALES_DIR} not found`)
  process.exit(1)
}

const catalogs = { en: readCatalog('en'), 'zh-CN': readCatalog('zh-CN') }
const problems = []
let checked = 0
let parameterised = 0

for (const file of sourceFiles(SRC_DIR)) {
  const text = readFileSync(file, 'utf8')
  const lines = text.split(/\r?\n/)
  for (const [index, line] of lines.entries()) {
    for (const match of line.matchAll(CALL)) {
      const key = match[1]
      if (!STATIC_KEY.test(key)) continue
      checked++
      for (const [locale, catalog] of Object.entries(catalogs)) {
        if (!catalog.has(key)) {
          problems.push(`${file}:${index + 1}  [${locale}] ${key}`)
        }
      }
    }
  }

  // A param name the entry does not interpolate prints a literal `{name}` on
  // screen, and a placeholder the call never fills does the same - both only in
  // the one language that has it, so nothing else notices.
  for (const match of text.matchAll(CALL_WITH_PARAMS)) {
    const [, key, params] = match
    const entry = catalogs.en.get(key)
    if (!STATIC_KEY.test(key) || typeof entry !== 'string') continue
    const expected = [...new Set([...entry.matchAll(PLACEHOLDER)].map((m) => m[1]))]
    const passed = match[2] === undefined ? [] : passedNames(match[2])
    parameterised++
    const missing = expected.filter((name) => !passed.includes(name))
    if (missing.length) {
      const line = text.slice(0, match.index).split('\n').length
      const how = match[2] === undefined ? 'passes no params, entry needs' : 'does not pass'
      problems.push(`${file}:${line}  ${key} ${how} ${missing.join(', ')}`)
    }
  }
}

if (problems.length) {
  console.log(problems.join('\n'))
  console.error(`\n::error::${problems.length} unresolved translation key(s) or param(s)`)
  console.error('Either add the key to both catalogs or fix the typo at the call site.')
  process.exit(1)
}

console.log(
  `i18n usage check passed: ${checked} static t() key(s) resolve in en and zh-CN; ` +
    `${parameterised} call(s) pass the params their entry interpolates`
)
