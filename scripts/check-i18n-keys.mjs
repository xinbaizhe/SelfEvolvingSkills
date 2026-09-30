#!/usr/bin/env node
//
// Fails when the `en` and `zh-CN` catalogs have drifted apart.
//
// Two things are compared, and they catch different mistakes:
//
// 1. Every key must exist in both languages. A key present only in `zh-CN`
//    shows English text in the Chinese UI (via fallbackLocale), and a key
//    present only in `en` renders as a raw key path for Chinese users. Neither
//    is caught by a type checker, because the catalogs are plain JSON.
//
// 2. Both languages must interpolate the same `{placeholders}`. A key whose
//    Chinese text says `{count}` and whose English text says `{total}` prints a
//    literal `{count}` on screen in whichever language is wrong - and only for
//    the users who read that language.
//
// Usage: node scripts/check-i18n-keys.mjs

import { readFileSync, readdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

// Resolved from this script's own location rather than the caller's cwd, so it
// behaves the same from the repo root, from frontend/, and in CI.
const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')

const ROOT = join(REPO_ROOT, 'frontend', 'src', 'i18n', 'locales')
const LOCALES = ['en', 'zh-CN']

/**
 * Flattens a nested catalog into a map of dotted key path to leaf value.
 *
 * Leaves keep their value rather than being reduced to a bare path, because the
 * placeholder check needs the text.
 */
function flatten(value, prefix, out) {
  const isBranch = value !== null && typeof value === 'object' && !Array.isArray(value)
  if (isBranch) {
    for (const [key, child] of Object.entries(value)) {
      flatten(child, prefix ? `${prefix}.${key}` : key, out)
    }
  } else {
    out.set(prefix, value)
  }
  return out
}

function readCatalog(locale, file) {
  const path = join(ROOT, locale, file)
  let parsed
  try {
    parsed = JSON.parse(readFileSync(path, 'utf8'))
  } catch (err) {
    console.error(`::error::${path} is not valid JSON - ${err.message}`)
    process.exit(1)
  }
  return flatten(parsed, '', new Map())
}

/** The `{name}` placeholders a catalog string interpolates. */
function placeholders(text) {
  if (typeof text !== 'string') return null
  return [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',')
}

const files = Object.fromEntries(
  LOCALES.map((locale) => [locale, readdirSync(join(ROOT, locale)).filter((f) => f.endsWith('.json')).sort()])
)

const [reference, ...others] = LOCALES
let failed = false

if (others.some((locale) => files[locale].join() !== files[reference].join())) {
  console.error(
    `::error::catalog file lists differ:\n  ${reference}: ${files[reference].join(', ')}\n` +
      others.map((l) => `  ${l}: ${files[l].join(', ')}`).join('\n')
  )
  failed = true
}

for (const file of files[reference]) {
  const keys = Object.fromEntries(LOCALES.map((l) => [l, readCatalog(l, file)]))

  for (const locale of LOCALES) {
    const counterpart = LOCALES.find((l) => l !== locale)
    const missing = [...keys[counterpart].keys()].filter((k) => !keys[locale].has(k))
    if (missing.length > 0) {
      console.error(`::error::${file}: ${missing.length} key(s) missing from ${locale}/`)
      for (const key of missing.slice(0, 20)) console.error(`  ${key}`)
      if (missing.length > 20) console.error(`  ... and ${missing.length - 20} more`)
      failed = true
    }
  }

  const mismatched = []
  for (const [key, en] of keys.en) {
    if (!keys['zh-CN'].has(key)) continue
    const theirs = placeholders(keys['zh-CN'].get(key))
    const ours = placeholders(en)
    if (theirs !== null && ours !== null && theirs !== ours) {
      mismatched.push(`  ${key}\n    en: ${ours || '(none)'}\n    zh-CN: ${theirs || '(none)'}`)
    }
  }
  if (mismatched.length > 0) {
    console.error(`::error::${file}: ${mismatched.length} key(s) interpolate different placeholders per language`)
    console.error(mismatched.slice(0, 20).join('\n'))
    if (mismatched.length > 20) console.error(`  ... and ${mismatched.length - 20} more`)
    failed = true
  }
}

if (failed) process.exit(1)
console.log(
  `i18n key check passed: ${LOCALES.join(' and ')} agree on keys and placeholders across ${files[reference].length} catalog(s)`
)
