import i18n from '../i18n'

/**
 * A backend failure carrying the catalog key for its own prose.
 *
 * Built by `utils::failure` on the Rust side. It is JSON encoded as a string
 * because the backend's error channel is `Result<_, String>` end to end.
 */
interface FailureEnvelope {
  code: string
  params?: Record<string, unknown>
  message?: string
}

/**
 * Reads a failure envelope, or returns null for anything else.
 *
 * Deliberately strict: the value has to be a string containing a JSON object
 * with a `code`. A plain sentence is the common case here - every backend
 * failure raised before this envelope existed, and every failure that has no
 * business being translated - and it must not be mistaken for one.
 */
function parseFailure(value: unknown): FailureEnvelope | null {
  if (typeof value !== 'string' || !value.trimStart().startsWith('{')) return null
  try {
    const parsed: unknown = JSON.parse(value)
    if (parsed === null || typeof parsed !== 'object') return null
    const { code, params, message } = parsed as Record<string, unknown>
    if (typeof code !== 'string') return null
    return {
      code,
      params: params !== null && typeof params === 'object' ? (params as Record<string, unknown>) : {},
      message: typeof message === 'string' ? message : undefined,
    }
  } catch {
    return null
  }
}

/**
 * Renders a failure envelope in the current language.
 *
 * The code is only trusted once this build is known to carry it. A key from a
 * newer backend would otherwise render as the literal key text, which is worse
 * than the untranslated prose it was meant to replace.
 */
function renderFailure(envelope: FailureEnvelope): string {
  const { t, te } = i18n.global
  if (te(envelope.code)) return t(envelope.code, envelope.params ?? {})
  return envelope.message ?? i18n.global.t('common.failed')
}

/**
 * Unwraps an unknown thrown value into a display string.
 *
 * Tauri rejects an `invoke` with the raw deserialised Rust error, so a backend
 * `Err(String)` arrives as a plain string rather than an `Error`. Behaviour for
 * everything that is not a failure envelope is unchanged: a translated
 * `fallback` when one was passed, the error's own message otherwise.
 *
 * `fallback` is resolved lazily so a caller that has no message of its own gets
 * the current language's generic failure text, while callers that pass their own
 * already-translated string keep it.
 */
export function getErrorMessage(e: unknown, fallback?: string): string {
  const failure = parseFailure(e)
  if (failure) return renderFailure(failure)
  if (e instanceof Error) return e.message
  return fallback ?? i18n.global.t('common.failed')
}

/**
 * The backend's own explanation of a failure, in the interface language when it
 * carries a key and verbatim when it does not.
 *
 * Unlike [`getErrorMessage`], a readable string is preferred over the fallback.
 * Use this where the backend is the only thing that knows *why* a call failed -
 * a rejected sign-in, a rejected URL - so a generic "operation failed" would
 * lose the one useful sentence in the exchange.
 */
export function describeError(e: unknown, fallback?: string): string {
  const failure = parseFailure(e)
  if (failure) return renderFailure(failure)
  if (typeof e === 'string' && e.trim()) return e
  if (e instanceof Error && e.message) return e.message
  return fallback ?? i18n.global.t('common.failed')
}

/**
 * The catalog key a failure carries, or null for anything else.
 *
 * For the callers that have to *branch* on which failure arrived - treating a
 * busy scan as informational, or a transport failure as "offline" - rather than
 * render it. Keying on the code keeps the decision independent of the language
 * the prose happens to be in.
 */
export function failureCode(e: unknown): string | null {
  return parseFailure(e)?.code ?? null
}
