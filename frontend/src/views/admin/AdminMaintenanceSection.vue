<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { DatabaseInfo, TableDetail } from '../../api/system'
import { getTableDetail, createTableRow, updateTableRow, deleteTableRow } from '../../api/system'
import type { useScanStore } from '../../stores/useScanStore'

interface DbTable {
  name: string
  label: string
  count: number
}

defineProps<{
  scanStore: ReturnType<typeof useScanStore>
  clearing: boolean
  initializing: boolean
  updateChecking: boolean
  dbInfo: DatabaseInfo | null
  dbTables: DbTable[]
  dbTotalRows: number
}>()

const emit = defineEmits<{
  (e: 'scan'): void
  (e: 'clearLogs'): void
  (e: 'initDb'): void
  (e: 'checkUpdate'): void
}>()

const detailVisible = ref(false)
const detailLoading = ref(false)
const detailLabel = ref('')
const detailData = ref<TableDetail | null>(null)
const detailPage = ref(1)
const detailSize = 10

const fieldLabelMap: Record<string, string> = {
  // Common
  id: 'ID',
  name: '名称',
  description: '描述',
  created_at: '创建时间',
  updated_at: '更新时间',
  // skills
  origin: '来源',
  source_type: '来源类型',
  agent_source: 'Agent 来源',
  plugin_name: '插件名',
  file_path: '文件路径',
  yaml_raw: 'YAML 原始数据',
  body_text: '正文',
  body_size: '正文大小',
  line_count: '行数',
  file_mtime: '文件修改时间',
  file_size: '文件大小',
  file_hash: '文件哈希',
  category: '分类',
  category_tags: '分类标签',
  usage_count: '使用次数',
  session_count: '会话次数',
  // agents
  tools: '工具列表',
  model: '模型',
  // sessions
  session_id: '会话 ID',
  pid: '进程 ID',
  cwd: '工作目录',
  project_name: '项目名',
  entrypoint: '入口',
  version: '版本',
  kind: '类型',
  started_at: '开始时间',
  message_count: '消息数',
  first_prompt: '首次提示词',
  compressed_summary: '压缩摘要',
  jsonl_path: 'JSONL 路径',
  jsonl_size: 'JSONL 大小',
  // memories
  mem_type: '记忆类型',
  origin_session_id: '来源会话 ID',
  // skill_usage
  skill_name: 'Skill 名',
  skill_source_type: 'Skill 来源类型',
  usage_type: '使用类型',
  mention_count: '引用次数',
  first_used_at: '首次使用时间',
  // scan_jobs
  scan_type: '扫描类型',
  status: '状态',
  completed_at: '完成时间',
  skills_found: '发现 Skills',
  agents_found: '发现 Agents',
  sessions_found: '发现会话数',
  conversations_analyzed: '分析会话数',
  memories_found: '发现记忆数',
  sources_scanned: '扫描源数',
  errors: '错误信息',
  config_snapshot: '配置快照',
  progress: '进度',
  message: '消息',
  data: '数据',
  phase: '阶段',
  // source_configs
  agent_id: 'Agent ID',
  agent_name: 'Agent 名',
  detected_path: '检测路径',
  custom_paths: '自定义路径',
  is_enabled: '是否启用',
  is_available: '是否可用',
  record_count: '记录数',
  last_activity: '最后活动',
  last_scan_at: '最后扫描时间',
  // admin_users
  username: '用户名',
  password_hash: '密码哈希',
  is_active: '是否活跃',
  // community_skills
  repo_full_name: '仓库全名',
  repo_url: '仓库 URL',
  stars: '星标数',
  skill_md_content: 'Skill 内容',
  file_url: '文件 URL',
  installed: '已安装',
  verified: '已验证',
  relevance_score: '相关性评分',
  quality_score: '质量评分',
  weighted_score: '加权评分',
  license: '许可证',
  pushed_at: '推送时间',
  matched_file: '匹配文件',
  readme_excerpt: 'README 摘要',
  source: '来源',
  recommendation_reason: '推荐理由',
  topic: '主题',
  fetched_at: '获取时间',
  // workflow_clusters
  frequency: '频率',
  source_agents: '来源 Agents',
  estimated_time_saved: '预估节省时间',
  can_generate_skill: '可生成 Skill',
  skill_score: 'Skill 评分',
  draft_body: '草案正文',
  sample_tasks: '示例任务',
  recommendation_source: '推荐来源',
  confidence: '置信度',
  reasoning: '推理说明',
  source_skills: '来源 Skills',
  similar_skills: '相似 Skills',
  review_score: '审核评分',
  review_summary: '审核摘要',
  review_feedback: '审核反馈',
  // evolution_jobs
  run_id: '运行 ID',
}

function fieldLabel(name: string): string {
  return fieldLabelMap[name] || name
}

async function openDetail(table: DbTable) {
  detailLabel.value = table.label
  detailPage.value = 1
  detailData.value = null
  detailVisible.value = true
  await loadDetail(table.name)
}

async function loadDetail(tableName: string) {
  detailLoading.value = true
  try {
    const res = await getTableDetail(tableName, undefined, detailPage.value, detailSize)
    if (res.success) {
      detailData.value = res.data
    }
  } finally {
    detailLoading.value = false
  }
}

async function onDetailPageChange(page: number) {
  detailPage.value = page
  if (detailData.value) {
    await loadDetail(detailData.value.table)
  }
}

function getStatusType(status: string) {
  if (status === 'completed') return 'success'
  if (status === 'failed') return 'danger'
  if (status === 'running') return 'warning'
  return 'info'
}

function getStatusLabel(status: string) {
  const labels: Record<string, string> = {
    completed: '已完成',
    failed: '失败',
    running: '运行中',
    pending: '待处理',
    cancelled: '已取消',
  }
  return labels[status] || status
}

function getScanTypeLabel(type: string) {
  const labels: Record<string, string> = {
    full: '全量扫描',
    incremental: '增量扫描',
    manual: '手动扫描',
  }
  return labels[type] || type
}

// ── Inline CRUD ──
const editingRowid = ref<number | null>(null)
const editingData = ref<Record<string, unknown>>({})
const addingNew = ref(false)
const newRowData = ref<Record<string, unknown>>({})
const saving = ref(false)

function startEdit(row: Record<string, unknown>) {
  editingRowid.value = row['rowid'] as number
  editingData.value = { ...row }
  addingNew.value = false
}

function cancelEdit() {
  editingRowid.value = null
  editingData.value = {}
}

async function saveEdit() {
  if (!detailData.value || editingRowid.value == null) return
  saving.value = true
  try {
    const res = await updateTableRow(detailData.value.table, editingRowid.value, editingData.value)
    if (res.success) {
      ElMessage.success('已更新')
      editingRowid.value = null
      editingData.value = {}
      await loadDetail(detailData.value.table)
    }
  } catch (e) { ElMessage.error(String(e)) }
  finally { saving.value = false }
}

function startAdd() {
  addingNew.value = true
  editingRowid.value = null
  newRowData.value = {}
  for (const col of detailData.value?.columns || []) {
    if (col.name !== 'rowid') newRowData.value[col.name] = ''
  }
}

function cancelAdd() {
  addingNew.value = false
  newRowData.value = {}
}

async function saveAdd() {
  if (!detailData.value) return
  saving.value = true
  try {
    const res = await createTableRow(detailData.value.table, newRowData.value)
    if (res.success) {
      ElMessage.success('已新增')
      addingNew.value = false
      newRowData.value = {}
      await loadDetail(detailData.value.table)
    }
  } catch (e) { ElMessage.error(String(e)) }
  finally { saving.value = false }
}

async function handleDelete(row: Record<string, unknown>) {
  if (!detailData.value) return
  try {
    await ElMessageBox.confirm('确定删除该行数据？此操作不可恢复。', '确认删除', { confirmButtonText: '删除', cancelButtonText: '取消', type: 'warning' })
  } catch { return }

  saving.value = true
  try {
    const rowid = row['rowid'] as number
    const res = await deleteTableRow(detailData.value.table, rowid)
    if (res.success) {
      ElMessage.success('已删除')
      await loadDetail(detailData.value.table)
    }
  } catch (e) { ElMessage.error(String(e)) }
  finally { saving.value = false }
}
</script>

<template>
  <div class="maintenance-grid">
    <div class="maintenance-item">
      <h4>扫描本机 Agent 数据</h4>
      <p>扫描 Skills、Agents、会话和记忆数据，并更新最近扫描记录。</p>
      <el-button type="primary" :loading="scanStore.scanning" @click="emit('scan')">{{ scanStore.scanning ? '扫描中...' : '开始扫描' }}</el-button>
    </div>
    <div class="maintenance-item">
      <h4>清理日志与历史</h4>
      <p>保留业务数据，只清理扫描历史、进化记录、聚类和使用记录。</p>
      <el-button type="warning" :loading="clearing" @click="emit('clearLogs')">清理日志与历史</el-button>
    </div>
    <div class="maintenance-item maintenance-item--danger">
      <h4>初始化数据库</h4>
      <p>清空 Skills、Agents、会话、社区缓存和历史记录，仅保留基础配置。</p>
      <el-button type="danger" :loading="initializing" @click="emit('initDb')">初始化数据库</el-button>
    </div>
  </div>

  <el-card class="history-card" shadow="never">
    <template #header>最近扫描历史</template>
    <el-table :data="scanStore.history" stripe max-height="360" empty-text="暂无扫描记录">
      <el-table-column prop="id" label="ID" width="60" />
      <el-table-column prop="scan_type" label="类型" width="100">
        <template #default="{ row }">{{ getScanTypeLabel(row.scan_type) }}</template>
      </el-table-column>
      <el-table-column prop="status" label="状态" width="110">
        <template #default="{ row }">
          <el-tag :type="getStatusType(row.status) as any">{{ getStatusLabel(row.status) }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="skills_found" label="Skills" width="90" />
      <el-table-column prop="agents_found" label="Agents" width="90" />
      <el-table-column prop="sessions_found" label="会话" width="90" />
      <el-table-column prop="conversations_analyzed" label="分析数" width="90" />
      <el-table-column prop="memories_found" label="记忆" width="80" />
      <el-table-column prop="started_at" label="开始时间" width="170" />
      <el-table-column prop="completed_at" label="完成时间" width="170" />
      <el-table-column prop="errors" label="错误" min-width="200">
        <template #default="{ row }">
          <span v-if="row.errors" class="error-text">{{ row.errors }}</span>
          <span v-else class="ok-text">无</span>
        </template>
      </el-table-column>
    </el-table>
  </el-card>

  <el-card class="history-card" shadow="never" v-if="dbInfo">
    <template #header>
      <div class="db-card-header">
        <span>数据库表统计</span>
        <span class="db-card-sub">共 {{ dbTables.length }} 张表 · {{ dbTotalRows.toLocaleString() }} 条记录</span>
      </div>
    </template>
    <div class="db-path">{{ dbInfo.db_path }}</div>
    <el-table :data="dbTables" stripe size="small" class="db-table">
      <el-table-column prop="label" label="表名" min-width="120" />
      <el-table-column prop="name" label="英文表名" min-width="120">
        <template #default="{ row }">
          <code class="db-table-code">{{ row.name }}</code>
        </template>
      </el-table-column>
      <el-table-column prop="count" label="记录数" min-width="120" sortable align="center">
        <template #default="{ row }">
          <span class="db-table-count">{{ row.count.toLocaleString() }}</span>
        </template>
      </el-table-column>
      <el-table-column label="操作" min-width="120" align="center">
        <template #default="{ row }">
          <el-button size="small" type="primary" link @click="openDetail(row)">查看详情</el-button>
        </template>
      </el-table-column>
    </el-table>
  </el-card>

  <el-dialog v-model="detailVisible" :title="`表数据: ${detailLabel}`" width="1060px" top="2vh" destroy-on-close @closed="detailData = null; editingRowid = null; addingNew = false">
    <template #header>
      <div style="display:flex;align-items:center;justify-content:space-between;width:100%">
        <span>表数据: {{ detailLabel }}</span>
        <el-button size="small" type="primary" @click="startAdd" :disabled="editingRowid != null">+ 新增一行</el-button>
      </div>
    </template>

    <!-- 数据表格 -->
    <el-table
      :data="detailData?.rows || []"
      stripe
      size="small"
      max-height="380"
      v-loading="detailLoading || saving"
      empty-text="暂无数据"
      class="detail-table"
    >
      <!-- Rowid column -->
      <el-table-column label="rowid" width="70" align="center">
        <template #default="{ row }">
          <span class="cell-rowid">{{ row['rowid'] }}</span>
        </template>
      </el-table-column>

      <!-- Data columns -->
      <el-table-column
        v-for="col in detailData?.columns || []"
        :key="col.cid"
        :prop="col.name"
        min-width="120"
        show-overflow-tooltip
      >
        <template #header>
          <div class="data-col-header">
            <code class="data-col-name">{{ col.name }}</code>
            <span class="data-col-label">{{ fieldLabel(col.name) }}</span>
          </div>
        </template>
        <template #default="{ row }">
          <template v-if="editingRowid === row['rowid']">
            <el-input
              v-model="editingData[col.name]"
              size="small"
              :placeholder="col.name"
            />
          </template>
          <span v-else class="cell-value">{{ row[col.name] ?? '' }}</span>
        </template>
      </el-table-column>

      <!-- Actions column -->
      <el-table-column label="操作" width="140" align="center" fixed="right">
        <template #default="{ row }">
          <template v-if="editingRowid === row['rowid']">
            <el-button size="small" type="success" :loading="saving" @click="saveEdit">保存</el-button>
            <el-button size="small" @click="cancelEdit">取消</el-button>
          </template>
          <template v-else>
            <el-button size="small" type="primary" link :disabled="editingRowid != null" @click="startEdit(row)">编辑</el-button>
            <el-button size="small" type="danger" link :disabled="editingRowid != null" @click="handleDelete(row)">删除</el-button>
          </template>
        </template>
      </el-table-column>
    </el-table>

    <!-- New row form -->
    <div v-if="addingNew" class="new-row-form">
      <h4>新增记录</h4>
      <div class="new-row-grid">
        <div v-for="col in (detailData?.columns || [])" :key="col.cid" class="new-row-field">
          <template v-if="col.name !== 'rowid'">
            <label>{{ fieldLabel(col.name) }} <code>{{ col.name }}</code></label>
            <el-input v-model="newRowData[col.name]" size="small" :placeholder="col.name" />
          </template>
        </div>
      </div>
      <div class="new-row-actions">
        <el-button type="primary" size="small" :loading="saving" @click="saveAdd">确认新增</el-button>
        <el-button size="small" @click="cancelAdd">取消</el-button>
      </div>
    </div>

    <div class="detail-footer" v-if="detailData">
      <span class="detail-total">共 {{ detailData.total }} 条</span>
      <el-pagination
        v-model:current-page="detailPage"
        :page-size="detailSize"
        :total="detailData.total"
        layout="prev, pager, next"
        size="small"
        background
        @current-change="onDetailPageChange"
      />
    </div>
  </el-dialog>
</template>

<style scoped>
.maintenance-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
}

.maintenance-item {
  position: relative;
  min-height: 100%;
  background: linear-gradient(180deg, #ffffff, #fbfdff);
  border: 1px solid #e2e8f0;
  border-radius: 10px;
  padding: 18px;
  box-shadow: 0 2px 12px rgba(15, 23, 42, 0.04);
  overflow: hidden;
  transition: transform 0.18s ease, box-shadow 0.18s ease, border-color 0.18s ease;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 14px;
  min-height: 158px;
}

.maintenance-item::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  background: linear-gradient(180deg, #14b8a6, #0891b2);
}

.maintenance-item:hover {
  transform: translateY(-2px);
  border-color: rgba(13, 148, 136, 0.24);
  box-shadow: 0 14px 34px rgba(15, 23, 42, 0.08);
}

.maintenance-item--danger {
  border-color: #fecaca;
  background: linear-gradient(180deg, #fff, #fffafa);
}

.maintenance-item--danger::before {
  background: linear-gradient(180deg, #ef4444, #f97316);
}

.maintenance-item h4 {
  margin: 0;
  color: #0f172a;
  font-size: 15px;
  font-weight: 800;
}

.maintenance-item p {
  margin: 0;
  color: #64748b;
  font-size: 13px;
  line-height: 1.55;
}

.history-card {
  margin-top: 16px;
  border: 1px solid #e6ebf2;
  border-radius: 10px;
  overflow: hidden;
}

.history-card :deep(.el-card__header) {
  padding: 14px 16px;
  color: #0f172a;
  font-weight: 800;
  background: #f8fafc;
}

.history-card :deep(.el-card__body) { padding: 0; }

.db-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.db-card-sub {
  color: #64748b;
  font-size: 12px;
  font-weight: 400;
}

.db-path {
  margin: 0;
  padding: 10px 16px;
  color: #64748b;
  font-size: 11px;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  background: #fbfdff;
  border-bottom: 1px solid #eef2f7;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.db-table :deep(.el-table__header-wrapper th) {
  background: #f8fafc;
  color: #475569;
  font-weight: 700;
}

.db-table-code {
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 12px;
  color: #475569;
  background: #f1f5f9;
  padding: 2px 6px;
  border-radius: 3px;
}

.db-table-count {
  color: #0f766e;
  font-weight: 800;
  font-size: 14px;
  font-variant-numeric: tabular-nums;
}

.error-text { color: #c24141; font-size: 12px; }
.ok-text { color: #0f766e; font-weight: 600; }

/* ---- 字段信息区域 ---- */
.detail-fields-card {
  margin-bottom: 16px;
  border: 1px solid #e8ecf2;
  border-radius: 8px;
  overflow: hidden;
}

.detail-fields-title {
  padding: 10px 14px;
  font-size: 13px;
  font-weight: 700;
  color: #475569;
  background: #f8fafc;
  border-bottom: 1px solid #eef2f7;
}

.fields-table :deep(.el-table__header-wrapper th) {
  background: #fafbfc;
  color: #64748b;
  font-weight: 600;
  font-size: 12px;
}

.field-name-code {
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 12px;
  color: #0f766e;
  background: #f0fdfa;
  padding: 2px 6px;
  border-radius: 3px;
}

.field-label-text {
  font-size: 13px;
  color: #334155;
}

.field-type-tag {
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 11px;
}

/* ---- 数据表格 ---- */
.detail-table {
  border: 1px solid #e8ecf2;
  border-radius: 8px;
  overflow: hidden;
}

.detail-table :deep(.el-table__header-wrapper th) {
  background: #f8fafc;
  padding: 8px 0;
}

.data-col-header {
  display: flex;
  flex-direction: column;
  gap: 1px;
  line-height: 1.3;
}

.data-col-name {
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 11px;
  color: #0f766e;
  font-weight: 700;
}

.data-col-label {
  font-size: 11px;
  color: #94a3b8;
  font-weight: 400;
}

.cell-value {
  font-size: 12px;
  color: #334155;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  white-space: nowrap;
}

.detail-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 14px;
}

.detail-total {
  font-size: 13px;
  color: #64748b;
}

.cell-rowid {
  font-family: ui-monospace, monospace;
  font-size: 11px;
  color: #94a3b8;
}

.new-row-form {
  margin-top: 16px;
  padding: 16px;
  border: 1.5px solid #0ea5e9;
  border-radius: 10px;
  background: #f0f9ff;
}

.new-row-form h4 {
  margin: 0 0 12px;
  font-size: 14px;
  color: #0c4a6e;
}

.new-row-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 10px;
}

.new-row-field label {
  display: block;
  font-size: 11px;
  color: #475569;
  margin-bottom: 3px;
}

.new-row-field label code {
  font-size: 10px;
  color: #94a3b8;
}

.new-row-actions {
  margin-top: 14px;
  display: flex;
  gap: 8px;
}

@media (max-width: 900px) {
  .maintenance-grid,
  .db-tables { grid-template-columns: 1fr; }
}
</style>
