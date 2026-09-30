<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { setLocale, type AppLocale } from '../../i18n'

const { t, locale } = useI18n()

const OPTIONS: { value: AppLocale; labelKey: string }[] = [
  { value: 'zh-CN', labelKey: 'locale.zh' },
  { value: 'en', labelKey: 'locale.en' },
]

function select(next: AppLocale) {
  if (next !== locale.value) setLocale(next)
}
</script>

<template>
  <div class="locale-switcher" role="group" :aria-label="t('locale.label')">
    <button
      v-for="option in OPTIONS"
      :key="option.value"
      type="button"
      class="locale-option"
      :class="{ active: locale === option.value }"
      :aria-pressed="locale === option.value"
      @click="select(option.value)"
    >{{ t(option.labelKey) }}</button>
  </div>
</template>

<style scoped>
.locale-switcher {
  display: inline-flex;
  border: 1px solid var(--line);
  border-radius: 10px;
  overflow: hidden;
  background: #fff;
}

.locale-option {
  font: inherit;
  font-size: 13px;
  font-weight: 500;
  line-height: 1;
  padding: 9px 10px;
  border: 0;
  background: transparent;
  color: var(--ink);
  cursor: pointer;
  transition: background .15s, color .15s;
}

.locale-option + .locale-option {
  border-left: 1px solid var(--line);
}

.locale-option:hover {
  color: var(--blue);
}

.locale-option.active {
  background: rgba(45, 212, 191, .12);
  color: #0f766e;
}
</style>
