<script setup lang="ts">
defineProps<{
  percent: number
  statusText: string
}>()

const circumference = 2 * Math.PI * 54
</script>

<template>
  <div class="progress-ring-container">
    <svg class="progress-ring" viewBox="0 0 120 120">
      <circle class="ring-bg" cx="60" cy="60" r="54" fill="none" stroke="var(--el-border-color-lighter)" stroke-width="8" />
      <circle
        class="ring-fill"
        cx="60"
        cy="60"
        r="54"
        fill="none"
        stroke="var(--el-color-primary)"
        stroke-width="8"
        stroke-linecap="round"
        :stroke-dasharray="circumference"
        :stroke-dashoffset="circumference - (percent / 100) * circumference"
        transform="rotate(-90 60 60)"
      />
      <text x="60" y="56" text-anchor="middle" class="ring-text-large">{{ percent }}%</text>
      <text x="60" y="74" text-anchor="middle" class="ring-text-small">{{ statusText }}</text>
    </svg>
  </div>
</template>

<style scoped>
.progress-ring-container {
  display: flex;
  justify-content: center;
  margin-bottom: 16px;
}

.progress-ring { width: 140px; height: 140px; }
.ring-bg { opacity: 0.15; }
.ring-fill { transition: stroke-dashoffset 0.6s ease; }
.ring-text-large { font-size: 22px; font-weight: 700; fill: var(--el-text-color-primary); }
.ring-text-small { font-size: 11px; fill: var(--el-text-color-secondary); }
</style>
