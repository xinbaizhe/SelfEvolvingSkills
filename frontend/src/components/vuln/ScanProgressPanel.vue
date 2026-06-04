<script setup lang="ts">
import { computed } from 'vue'
import type { VulnScanJob } from '../../api/vuln'

const props = defineProps<{
  scanning?: boolean
  result?: VulnScanJob | null
  progressMessages?: string[]
}>()

const scanSteps = [
  '校验目标',
  '加载登录态',
  '策略配置',
  '爬取入口',
  'SQL/XSS/SSRF等注入检测',
  '敏感路径/端口检测',
  'TLS/证书检查',
  '系统漏洞检测',
  'AI 多角色智能发现',
  'AI 多角色复核',
  '保存结果',
]

const scanStepDescriptions: Record<string, string> = {
  '校验目标': '校验 URL、目录路径、模型配置和扫描参数是否可用。',
  '加载登录态': '装载 Cookie、Authorization 和自定义请求头。',
  '策略配置': '按快速、标准、深度模式确定爬取深度和 payload 范围。',
  '爬取入口': '请求目标页面并收集同源链接、表单、参数和可测试入口。',
  'SQL/XSS/SSRF等注入检测': '对 URL 参数和表单执行 SQL/XSS/SSRF/NoSQL/SSTI/LFI 全部注入类型检测。',
  '敏感路径/端口检测': '探测常见敏感路径，扫描目标 IP 开放端口并识别服务。',
  'TLS/证书检查': '检查 HTTPS 证书有效性、TLS 协议版本等传输层安全问题。',
  '系统漏洞检测': 'HTTP方法/CRLF/Host头/默认凭据爆破/源码泄露等系统漏洞检测。',
  'AI 多角色智能发现': '6角色并行分析原始响应，发现规则扫描遗漏的漏洞。',
  'AI 多角色复核': '6角色并行复核，去重汇总确认漏洞。',
  '保存结果': '保存扫描结果并统计分级数量。',
}

const stepKeywords: Record<string, string[]> = {
  '校验目标': ['校验目标', '规范化 URL', '目标'],
  '加载登录态': ['加载登录态', 'Cookie', 'Authorization'],
  '策略配置': ['扫描策略', '快速扫描', '标准扫描', '深度扫描'],
  '爬取入口': ['爬取页面', '爬取入口', '读取源码文件'],
  'SQL/XSS/SSRF等注入检测': ['SQL 注入', 'XSS', 'SSRF', 'NoSQL', 'SSTI', 'LFI', 'SQL错误', '布尔盲注', '模板注入', '文件包含'],
  '敏感路径/端口检测': ['敏感路径', '端口扫描', '开放端口', 'TCP'],
  'TLS/证书检查': ['TLS', 'HTTPS', '证书', 'SSL'],
  '系统漏洞检测': ['系统漏洞检测', 'HTTP 方法', 'CRLF', 'Host头', '默认凭据', '源码泄露', '误报控制'],
  'AI 多角色智能发现': ['AI 智能发现', '6角色并行分析'],
  'AI 多角色复核': ['AI 复核', '6角色并行复核', '模型复核'],
  '保存结果': ['保存结果', '扫描完成'],
}

function progressLines(text?: string): string[] {
  if (!text) return []
  return text.split('\n').map(line => line.trim()).filter(Boolean)
}

function parseProgressMessage(msg: string): { step: number; text: string } | null {
  try {
    const json = JSON.parse(msg)
    if (typeof json.step === 'number' && typeof json.msg === 'string') {
      return { step: json.step, text: json.msg }
    }
  } catch {}
  return null
}

interface StepGroup {
  step: string
  index: number
  status: 'done' | 'running' | 'pending'
  description: string
  lines: string[]
}

const groups = computed<StepGroup[]>(() => {
  const messages = props.progressMessages || []
  const allText = props.result?.progressText || messages.join('\n')
  const lines = progressLines(allText)

  return scanSteps.map((step, idx) => {
    const hasMatch = lines.some(line => {
      const kws = stepKeywords[step] || []
      return kws.some(kw => line.includes(kw))
    })
    const stepLines = lines.filter(line => {
      const kws = stepKeywords[step] || []
      return kws.some(kw => line.includes(kw))
    })
    let status: 'done' | 'running' | 'pending' = 'pending'
    if (props.result) {
      status = 'done'
    } else if (props.scanning && hasMatch) {
      let nextIdx = -1
      for (let i = scanSteps.length - 1; i > idx; i--) {
        if (lines.some(l => (stepKeywords[scanSteps[i]] || []).some(kw => l.includes(kw)))) {
          nextIdx = i
          break
        }
      }
      status = nextIdx === -1 ? 'running' : 'done'
    }
    return { step, index: idx, status, description: scanStepDescriptions[step] || '', lines: props.result ? stepLines : stepLines.slice(0, 8) }
  })
})

const percent = computed(() => {
  if (props.result) return 100
  if (props.scanning) {
    const done = groups.value.filter(g => g.status === 'done').length
    const running = groups.value.some(g => g.status === 'running') ? 0.5 : 0
    return Math.min(99, Math.max(5, Math.round(((done + running) / scanSteps.length) * 100)))
  }
  return 0
})
</script>

<template>
  <div v-if="scanning || result" class="scan-progress-panel">
    <div class="scan-progress-header">
      <div>
        <h3>{{ result ? '扫描完成' : '扫描进行中' }}</h3>
        <p v-if="result">{{ result.target }}</p>
      </div>
      <el-tag :type="result ? 'success' : 'warning'" effect="light">
        {{ result ? '已完成' : `进行中 ${percent}%` }}
      </el-tag>
    </div>
    <div class="progress-bar-track">
      <div class="progress-bar-fill" :style="{ width: percent + '%' }"></div>
    </div>
    <div class="scan-progress-grid">
      <section
        v-for="group in groups"
        :key="group.step"
        class="scan-step-card"
        :class="`scan-step-${group.status}`"
      >
        <div class="scan-step-head">
          <span class="scan-step-index">{{ group.index + 1 }}</span>
          <div>
            <h4>{{ group.step }}</h4>
            <p>{{ group.description }}</p>
          </div>
          <el-tag v-if="group.status === 'done'" size="small" type="success">完成</el-tag>
          <el-tag v-else-if="group.status === 'running'" size="small" type="warning">进行中</el-tag>
          <el-tag v-else size="small" type="info">等待</el-tag>
        </div>
        <div class="scan-step-lines">
          <div v-if="group.lines.length === 0" class="scan-step-empty">暂无明细</div>
          <div v-for="(line, i) in group.lines" :key="i" class="scan-step-line">
            <span class="scan-step-dot"></span>
            <span>{{ line }}</span>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>
