<script setup lang="ts">
import type { EvolutionStep } from '../../api/evolution'

defineProps<{
  steps: EvolutionStep[]
  phaseStatuses: Map<string, string>
  currentPhase: string | null
  isRunning: boolean
}>()

function phaseCompleted(phase: string, phaseStatuses: Map<string, string>): boolean {
  return phaseStatuses.get(phase) === 'completed'
}

function phaseActive(phase: string, currentPhase: string | null, isRunning: boolean): boolean {
  return currentPhase === phase && isRunning
}

function phaseFailed(phase: string, phaseStatuses: Map<string, string>): boolean {
  return phaseStatuses.get(phase) === 'failed'
}
</script>

<template>
  <div class="steps-bar">
    <div
      v-for="(step, index) in steps"
      :key="step.phase"
      class="step-item"
      :class="{
        active: phaseActive(step.phase, currentPhase, isRunning),
        completed: phaseCompleted(step.phase, phaseStatuses),
        failed: phaseFailed(step.phase, phaseStatuses),
      }"
    >
      <div class="step-dot">
        <span v-if="phaseCompleted(step.phase, phaseStatuses)" class="check">&#10003;</span>
        <span v-else-if="phaseFailed(step.phase, phaseStatuses)" class="cross">&#10007;</span>
        <span v-else-if="phaseActive(step.phase, currentPhase, isRunning)" class="pulse" />
        <span v-else class="num">{{ index + 1 }}</span>
      </div>
      <div class="step-label">{{ step.label }}</div>
      <div class="step-range">{{ step.start }}% - {{ step.end }}%</div>
    </div>
  </div>
</template>

<style scoped>
.steps-bar {
  display: flex;
  justify-content: space-between;
  gap: 4px;
}

.step-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  flex: 1;
  position: relative;
}

.step-item::after {
  content: '';
  position: absolute;
  top: 14px;
  left: 60%;
  right: -40%;
  height: 2px;
  background: var(--el-border-color-lighter);
  z-index: 0;
}

.step-item:last-child::after { display: none; }
.step-item.completed::after { background: var(--el-color-primary); }

.step-dot {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 600;
  z-index: 1;
  border: 2px solid var(--el-border-color);
  background: var(--el-bg-color);
  color: var(--el-text-color-secondary);
  transition: all 0.3s;
}

.step-item.active .step-dot {
  border-color: var(--el-color-primary);
  color: var(--el-color-primary);
  box-shadow: 0 0 0 4px var(--el-color-primary-light-8);
}

.step-item.completed .step-dot {
  border-color: var(--el-color-primary);
  background: var(--el-color-primary);
  color: #fff;
}

.step-item.failed .step-dot {
  border-color: var(--el-color-danger);
  background: var(--el-color-danger);
  color: #fff;
}

.step-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  white-space: nowrap;
}

.step-item.active .step-label,
.step-item.completed .step-label {
  color: var(--el-color-primary);
  font-weight: 600;
}

.step-item.failed .step-label {
  color: var(--el-color-danger);
}

.step-range {
  font-size: 10px;
  color: var(--el-text-color-placeholder);
}

.check { font-size: 14px; color: #fff; }
.cross { font-size: 14px; color: #fff; font-weight: 700; }
.pulse {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--el-color-primary);
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.4; transform: scale(0.7); }
}
</style>
