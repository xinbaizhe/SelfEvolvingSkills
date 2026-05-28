<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { useTeamStore } from '../../stores/useTeamStore'
import { shareSkillToTeam } from '../../api/team'
import { fetchSkills, fetchSkillDetail, type SkillItem } from '../../api/skills'
import { getErrorMessage } from '../../utils/error'
import { TEAM_CATEGORY_OPTIONS } from '../../constants/team'
import { createStoredZip, downloadBlob, sanitizeZipSegment } from '../../utils/zip'

const store = useTeamStore()
const visible = ref(false)
const loading = ref(false)
const sharing = ref(false)

const form = reactive({
  shareType: 'skill' as 'skill' | 'url',
  name: '',
  url: '',
  description: '',
  category: 'general',
  bodyMd: '',
  originAgent: '',
  compatibleModels: '',
  compatibleAgents: '',
})

const localSkills = ref<SkillItem[]>([])
const selectedSkillKey = ref('')
const downloadAfterShare = ref(true)

const zipFilename = computed(() => `${sanitizeZipSegment(form.name)}.zip`)

const emit = defineEmits<{
  (e: 'shared'): void
}>()

async function loadLocalSkills() {
  try {
    const res = await fetchSkills({ size: 200 })
    if (res.success && res.data) {
      localSkills.value = res.data.items ?? []
    }
  } catch { /* 本地 Skill 列表加载失败不阻塞手工填写 */ }
}

watch(selectedSkillKey, async (key) => {
  if (!key) return
  loading.value = true
  try {
    const [sourceType, name] = key.split(':', 2)
    const res = await fetchSkillDetail(name, sourceType)
    if (res.success && res.data) {
      const detail = res.data
      form.name = detail.name
      form.description = detail.description || ''
      form.category = detail.category || 'general'
      form.bodyMd = detail.body_text || ''
      form.originAgent = detail.source_type || ''
    }
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '加载 Skill 详情失败'))
  } finally {
    loading.value = false
  }
})

function resetForm() {
  form.shareType = 'skill'
  form.name = ''
  form.url = ''
  form.description = ''
  form.category = 'general'
  form.bodyMd = ''
  form.originAgent = ''
  form.compatibleModels = ''
  form.compatibleAgents = ''
  selectedSkillKey.value = ''
  downloadAfterShare.value = true
}

function open() {
  if (!store.isAuthenticated) {
    ElMessage.warning('请先登录团队版')
    return
  }
  resetForm()
  visible.value = true
  loadLocalSkills()
}

function buildZip() {
  const dir = sanitizeZipSegment(form.name)
  return createStoredZip([
    { path: `${dir}/SKILL.md`, content: form.bodyMd },
  ])
}

function buildPayloadBody() {
  if (form.shareType === 'url') {
    return [
      `# ${form.name}`,
      '',
      `URL: ${form.url}`,
      '',
      form.description || '',
    ].join('\n').trim()
  }
  return form.bodyMd
}

function isValidHttpUrl(value: string) {
  try {
    const url = new URL(value)
    return url.protocol === 'http:' || url.protocol === 'https:'
  } catch {
    return false
  }
}

function downloadCurrentZip() {
  if (form.shareType !== 'skill') {
    ElMessage.warning('工具网址不需要下载 zip')
    return
  }
  if (!form.name || !form.bodyMd) {
    ElMessage.warning('请先选择 Skill，确认名称和内容不为空')
    return
  }
  downloadBlob(buildZip(), zipFilename.value)
}

async function doShare() {
  if (!form.name || (form.shareType === 'skill' && !form.bodyMd) || (form.shareType === 'url' && !form.url)) {
    ElMessage.warning(form.shareType === 'url' ? '请填写名称和工具网址' : '请选择 Skill，并确认内容不为空')
    return
  }
  if (form.shareType === 'url' && !isValidHttpUrl(form.url)) {
    ElMessage.warning('请输入 http 或 https 开头的有效网址')
    return
  }
  sharing.value = true
  try {
    await shareSkillToTeam({
      name: form.name,
      description: form.description,
      category: form.category || 'general',
      sourceType: form.shareType,
      bodyMd: buildPayloadBody(),
      originAgent: form.shareType === 'url' ? 'tool-url' : (form.originAgent || undefined),
      compatibleModels: form.compatibleModels || undefined,
      compatibleAgents: form.compatibleAgents || undefined,
    })
    if (form.shareType === 'skill' && downloadAfterShare.value) downloadBlob(buildZip(), zipFilename.value)
    ElMessage.success(form.shareType === 'url' ? '工具网址已分享到团队' : 'Skill 已分享到团队')
    visible.value = false
    emit('shared')
  } catch (e: unknown) {
    ElMessage.error(getErrorMessage(e, '分享失败'))
  } finally {
    sharing.value = false
  }
}

defineExpose({ open })
</script>

<template>
  <el-dialog v-model="visible" title="分享 Skill 到团队" width="640px" top="8vh">
    <el-form label-position="top" @submit.prevent="doShare">
      <el-alert
        class="share-tip"
        type="info"
        :closable="false"
        show-icon
        title="可以分享完整 Skill，也可以分享团队常用工具网址。Skill 可同时下载为 zip，网址会作为团队资源展示。"
      />

      <el-form-item label="分享类型">
        <el-segmented
          v-model="form.shareType"
          :options="[
            { label: 'Skill zip', value: 'skill' },
            { label: '工具网址', value: 'url' },
          ]"
        />
      </el-form-item>

      <el-form-item v-if="form.shareType === 'skill'" label="选择本地 Skill">
        <el-select
          v-model="selectedSkillKey"
          placeholder="选择要分享的 Skill..."
          filterable
          style="width: 100%"
          :loading="loading"
        >
          <el-option
            v-for="skill in localSkills"
            :key="`${skill.source_type}:${skill.name}`"
            :label="skill.name"
            :value="`${skill.source_type}:${skill.name}`"
          >
            <span>{{ skill.name }}</span>
            <span class="option-meta">{{ skill.category || 'general' }} · {{ skill.source_type }}</span>
          </el-option>
        </el-select>
      </el-form-item>

      <div class="form-grid">
        <el-form-item label="名称">
          <el-input v-model="form.name" placeholder="Skill 名称" />
        </el-form-item>

        <el-form-item label="分类">
          <el-select v-model="form.category" style="width: 100%">
            <el-option v-for="item in TEAM_CATEGORY_OPTIONS" :key="item.value" :label="item.label" :value="item.value" />
          </el-select>
        </el-form-item>
      </div>

      <el-form-item v-if="form.shareType === 'url'" label="工具网址">
        <el-input v-model="form.url" placeholder="https://example.com/tool" />
      </el-form-item>

      <el-form-item label="描述">
        <el-input v-model="form.description" type="textarea" :rows="2" placeholder="简短说明这个 Skill 解决什么问题" />
      </el-form-item>

      <div v-if="form.shareType === 'skill'" class="form-grid">
        <el-form-item label="来源 Agent">
          <el-input v-model="form.originAgent" placeholder="如 codex、claude-code、cursor" />
        </el-form-item>

        <el-form-item label="兼容模型">
          <el-input v-model="form.compatibleModels" placeholder="逗号分隔，如 gpt-5, *" />
        </el-form-item>
      </div>

      <el-form-item v-if="form.shareType === 'skill'" label="兼容 Agent">
        <el-input v-model="form.compatibleAgents" placeholder="逗号分隔，如 codex, claude-code, cursor" />
      </el-form-item>

      <el-form-item v-if="form.shareType === 'skill'" label="Skill 内容">
        <el-input
          v-model="form.bodyMd"
          type="textarea"
          :rows="8"
          placeholder="SKILL.md 内容"
        />
      </el-form-item>

      <el-checkbox v-if="form.shareType === 'skill'" v-model="downloadAfterShare">提交成功后同时下载 zip</el-checkbox>
    </el-form>

    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button v-if="form.shareType === 'skill'" :disabled="!form.name || !form.bodyMd" @click="downloadCurrentZip">下载 zip</el-button>
      <el-button type="primary" :loading="sharing" @click="doShare">提交分享</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.share-tip {
  margin-bottom: 16px;
}

.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.option-meta {
  float: right;
  color: var(--muted);
  font-size: 12px;
}

@media (max-width: 640px) {
  .form-grid {
    grid-template-columns: 1fr;
  }
}
</style>
