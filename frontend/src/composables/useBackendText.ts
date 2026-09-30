import { useI18n } from 'vue-i18n'

/**
 * A piece of backend prose as the backend sends it.
 *
 * The backend writes it in three parts: `code`, the catalog key for the text;
 * `params`, the values filling that entry's placeholders; and `message`, the
 * Chinese prose. The prose is kept because a key this build does not carry
 * still has to render as something readable, and because the stored rows and
 * the logs should stay legible on their own.
 */
export interface BackendText {
  code?: string | null
  params?: Record<string, unknown> | null
  message?: string | null
  /**
   * A phrase appended to `message` rather than a message of its own.
   *
   * Only the evolution pipeline uses this: a stale-job sweep or a manual reset
   * changes a phase's fate without writing a message, so it tags the message
   * already there.
   */
  suffix_code?: string | null
}

/**
 * Anything the backend may hand over for rendering.
 *
 * A live event carries an envelope object. A stored column carries the same
 * envelope JSON-encoded in a `TEXT` column, which arrives as a string; plain
 * prose - a row written before the code columns existed, or a model's own
 * output - arrives as a string too and is rendered verbatim.
 */
export type BackendTextInput = BackendText | string | null | undefined

/** Reads a stored envelope string, or returns null for anything else. */
function parseEnvelope(value: string): BackendText | null {
  if (!value.trimStart().startsWith('{')) return null
  try {
    const parsed: unknown = JSON.parse(value)
    if (parsed === null || typeof parsed !== 'object') return null
    const { code, params, message, suffix_code: suffix } = parsed as Record<string, unknown>
    if (typeof code !== 'string' && typeof message !== 'string') return null
    return {
      code: typeof code === 'string' ? code : null,
      params: params !== null && typeof params === 'object' ? (params as Record<string, unknown>) : {},
      message: typeof message === 'string' ? message : undefined,
      suffix_code: typeof suffix === 'string' ? suffix : null,
    }
  } catch {
    return null
  }
}

/** Whether a param value is itself backend prose that must be rendered first. */
function isEnvelope(value: unknown): value is BackendText {
  return value !== null && typeof value === 'object' && typeof (value as BackendText).code === 'string'
}

/**
 * Renders backend prose in the interface language.
 *
 * The code wins whenever this build knows it; `message` is the fallback in the
 * two cases where there is no usable code - a key this frontend does not carry,
 * and a row recorded before the code column existed.
 *
 * A param may itself be an envelope: a sentence the backend builds from
 * variable clauses keeps each clause translatable, so a nested envelope is
 * rendered in the interface language before it fills the outer placeholder.
 *
 * A suffix's catalog entry carries its own leading space, matching the SQL that
 * concatenates it, so the two halves join by plain concatenation and stay in
 * the same language as the message they follow.
 */
export function useBackendText(): (text: BackendTextInput) => string {
  const { t, te } = useI18n()

  function resolveParams(params: Record<string, unknown>): Record<string, unknown> {
    const resolved: Record<string, unknown> = {}
    for (const [name, value] of Object.entries(params)) {
      resolved[name] = isEnvelope(value) ? render(value) : value
    }
    return resolved
  }

  function render(text: BackendText): string {
    const key = text.code
    const rendered = key && te(key) ? t(key, resolveParams(text.params ?? {})) : text.message || ''
    const suffix = text.suffix_code
    return suffix && te(suffix) ? `${rendered}${t(suffix)}` : rendered
  }

  return (text: BackendTextInput): string => {
    if (typeof text === 'string') {
      const envelope = parseEnvelope(text)
      return envelope ? render(envelope) : text
    }
    return text ? render(text) : ''
  }
}
