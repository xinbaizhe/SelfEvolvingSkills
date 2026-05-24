const EVENT_NAME = 'skills-changed'

export function emitSkillsChanged() {
  window.dispatchEvent(new CustomEvent(EVENT_NAME))
}

export function onSkillsChanged(callback: () => void) {
  window.addEventListener(EVENT_NAME, callback)
  return () => window.removeEventListener(EVENT_NAME, callback)
}
