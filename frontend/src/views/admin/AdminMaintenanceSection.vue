<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { DatabaseInfo, TableDetail } from '../../api/system'
import { getTableDetail, createTableRow, updateTableRow, deleteTableRow } from '../../api/system'
import type { useScanStore } from '../../stores/useScanStore'
import { describeError } from '../../utils/error'

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

const { t, te } = useI18n()

const detailVisible = ref(false)
const detailLoading = ref(false)
const detailLabel = ref('')
const detailData = ref<TableDetail | null>(null)
const detailPage = ref(1)
const detailSize = 10

// DB column name -> admin.maintenance.field.<camelCase>
function fieldLabel(name: string): string {
  const key = name.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase())
  const full = `admin.maintenance.field.${key}`
  return te(full) ? t(full) : name
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
    } else {
      ElMessage.error(t('admin.maintenance.loadDetailFailed'))
    }
  } catch (e) {
    ElMessage.error(t('admin.maintenance.loadDetailError', { error: String(e) }))
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
  const key = `admin.common.status.${status}`
  return te(key) ? t(key) : status
}

function getScanTypeLabel(type: string) {
  const key = `admin.common.scanType.${type}`
  return te(key) ? t(key) : type
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
  if (!detailData.value || editingRowid.value == null || editingRowid.value <= 0) {
    ElMessage.warning(t('admin.maintenance.rowidMissing'))
    return
  }
  saving.value = true
  try {
    const res = await updateTableRow(detailData.value.table, editingRowid.value, editingData.value)
    if (res.success) {
      ElMessage.success(t('admin.maintenance.updated'))
      editingRowid.value = null
      editingData.value = {}
      await loadDetail(detailData.value.table)
    } else {
      ElMessage.error(t('admin.maintenance.updateFailed'))
    }
  } catch (e) { ElMessage.error(t('admin.maintenance.updateError', { error: describeError(e) })) }
  finally { saving.value = false }
}

function startAdd() {
  addingNew.value = true
  editingRowid.value = null
  newRowData.value = {}
  for (const col of detailData.value?.columns || []) {
    if (col.name !== 'rowid' && col.name !== 'id') newRowData.value[col.name] = ''
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
      ElMessage.success(t('admin.maintenance.added'))
      addingNew.value = false
      newRowData.value = {}
      await loadDetail(detailData.value.table)
    } else {
      ElMessage.error(t('admin.maintenance.addFailed'))
    }
  } catch (e) { ElMessage.error(t('admin.maintenance.addError', { error: describeError(e) })) }
  finally { saving.value = false }
}

async function handleDelete(row: Record<string, unknown>) {
  if (!detailData.value) return
  const rowid = row['rowid'] as number
  if (!rowid || rowid <= 0) {
    ElMessage.warning(t('admin.maintenance.rowidMissing'))
    return
  }
  try {
    await ElMessageBox.confirm(t('admin.maintenance.deleteConfirm'), t('admin.maintenance.deleteTitle'), { confirmButtonText: t('common.delete'), cancelButtonText: t('common.cancel'), type: 'warning' })
  } catch { return }

  saving.value = true
  try {
    const res = await deleteTableRow(detailData.value.table, rowid)
    if (res.success) {
      ElMessage.success(t('admin.maintenance.deleted'))
      await loadDetail(detailData.value.table)
    } else {
      ElMessage.error(t('admin.maintenance.deleteFailed'))
    }
  } catch (e) { ElMessage.error(t('admin.maintenance.deleteError', { error: describeError(e) })) }
  finally { saving.value = false }
}
</script>

<template>
  <div class="maintenance-grid">
    <div class="maintenance-item">
      <h4>{{ t('admin.maintenance.scanTitle') }}</h4>
      <p>{{ t('admin.maintenance.scanDesc') }}</p>
      <el-button type="primary" :loading="scanStore.scanning" @click="emit('scan')">{{ scanStore.scanning ? t('admin.maintenance.scanning') : t('admin.maintenance.startScan') }}</el-button>
    </div>
    <div class="maintenance-item">
      <h4>{{ t('admin.maintenance.clearTitle') }}</h4>
      <p>{{ t('admin.maintenance.clearDesc') }}</p>
      <el-button type="warning" :loading="clearing" @click="emit('clearLogs')">{{ t('admin.maintenance.clearLogs') }}</el-button>
    </div>
    <div class="maintenance-item maintenance-item--danger">
      <h4>{{ t('admin.maintenance.initTitle') }}</h4>
      <p>{{ t('admin.maintenance.initDesc') }}</p>
      <el-button type="danger" :loading="initializing" @click="emit('initDb')">{{ t('admin.maintenance.initDb') }}</el-button>
    </div>
  </div>

  <el-card class="history-card" shadow="never">
    <template #header>{{ t('admin.maintenance.historyTitle') }}</template>
    <el-table :data="scanStore.history" stripe max-height="360" :empty-text="t('admin.maintenance.emptyHistory')">
      <el-table-column prop="id" label="ID" width="60" />
      <el-table-column prop="scan_type" :label="t('admin.common.type')" width="100">
        <template #default="{ row }">{{ getScanTypeLabel(row.scan_type) }}</template>
      </el-table-column>
      <el-table-column prop="status" :label="t('admin.common.statusLabel')" width="110">
        <template #default="{ row }">
          <el-tag :type="getStatusType(row.status) as any">{{ getStatusLabel(row.status) }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="skills_found" label="Skills" width="90" />
      <el-table-column prop="agents_found" label="Agents" width="90" />
      <el-table-column prop="sessions_found" :label="t('admin.maintenance.sessionsColumn')" width="90" />
      <el-table-column prop="conversations_analyzed" :label="t('admin.maintenance.analyzedColumn')" width="90" />
      <el-table-column prop="memories_found" :label="t('admin.maintenance.memoriesColumn')" width="80" />
      <el-table-column prop="started_at" :label="t('admin.common.startedAt')" width="170" />
      <el-table-column prop="completed_at" :label="t('admin.common.completedAt')" width="170" />
      <el-table-column prop="errors" :label="t('admin.maintenance.errorsColumn')" min-width="200">
        <template #default="{ row }">
          <span v-if="row.errors" class="error-text">{{ row.errors }}</span>
          <span v-else class="ok-text">{{ t('admin.maintenance.none') }}</span>
        </template>
      </el-table-column>
    </el-table>
  </el-card>

  <el-card class="history-card" shadow="never" v-if="dbInfo">
    <template #header>
      <div class="db-card-header">
        <span>{{ t('admin.maintenance.dbStatsTitle') }}</span>
        <span class="db-card-sub">{{ t('admin.maintenance.dbStatsSub', { tables: dbTables.length, rows: dbTotalRows.toLocaleString() }) }}</span>
      </div>
    </template>
    <div class="db-path">{{ dbInfo.db_path }}</div>
    <el-table :data="dbTables" stripe size="small" class="db-table">
      <el-table-column prop="label" :label="t('admin.maintenance.tableName')" min-width="120" />
      <el-table-column prop="name" :label="t('admin.maintenance.tableNameEn')" min-width="120">
        <template #default="{ row }">
          <code class="db-table-code">{{ row.name }}</code>
        </template>
      </el-table-column>
      <el-table-column prop="count" :label="t('admin.common.records')" min-width="120" sortable align="center">
        <template #default="{ row }">
          <span class="db-table-count">{{ row.count.toLocaleString() }}</span>
        </template>
      </el-table-column>
      <el-table-column :label="t('admin.common.action')" min-width="120" align="center">
        <template #default="{ row }">
          <el-button size="small" type="primary" link @click="openDetail(row)">{{ t('admin.maintenance.viewDetail') }}</el-button>
        </template>
      </el-table-column>
    </el-table>
  </el-card>

  <el-dialog v-model="detailVisible" :title="t('admin.maintenance.dialogTitle', { label: detailLabel })" width="1060px" top="2vh" destroy-on-close @closed="detailData = null; editingRowid = null; addingNew = false">
    <template #header>
      <div style="display:flex;align-items:center;justify-content:space-between;width:100%">
        <span>{{ t('admin.maintenance.dialogTitle', { label: detailLabel }) }}</span>
        <el-button size="small" type="primary" @click="startAdd" :disabled="editingRowid != null">{{ t('admin.maintenance.addRow') }}</el-button>
      </div>
    </template>

    <!-- 数据表格 -->
    <el-table
      :data="detailData?.rows || []"
      stripe
      size="small"
      max-height="380"
      v-loading="detailLoading || saving"
      :empty-text="t('common.empty')"
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
      <el-table-column :label="t('admin.common.action')" width="140" align="center" fixed="right">
        <template #default="{ row }">
          <template v-if="editingRowid === row['rowid']">
            <el-button size="small" type="success" :loading="saving" @click="saveEdit">{{ t('common.save') }}</el-button>
            <el-button size="small" @click="cancelEdit">{{ t('common.cancel') }}</el-button>
          </template>
          <template v-else>
            <el-button size="small" type="primary" link :disabled="editingRowid != null" @click="startEdit(row)">{{ t('common.edit') }}</el-button>
            <el-button size="small" type="danger" link :disabled="editingRowid != null" @click="handleDelete(row)">{{ t('common.delete') }}</el-button>
          </template>
        </template>
      </el-table-column>
    </el-table>

    <!-- New row form -->
    <div v-if="addingNew" class="new-row-form">
      <h4>{{ t('admin.maintenance.newRowTitle') }}</h4>
      <div class="new-row-grid">
        <div v-for="col in (detailData?.columns || [])" :key="col.cid" class="new-row-field">
          <template v-if="col.name !== 'rowid'">
            <label>{{ fieldLabel(col.name) }} <code>{{ col.name }}</code></label>
            <el-input v-model="newRowData[col.name]" size="small" :placeholder="col.name" />
          </template>
        </div>
      </div>
      <div class="new-row-actions">
        <el-button type="primary" size="small" :loading="saving" @click="saveAdd">{{ t('admin.maintenance.confirmAdd') }}</el-button>
        <el-button size="small" @click="cancelAdd">{{ t('common.cancel') }}</el-button>
      </div>
    </div>

    <div class="detail-footer" v-if="detailData">
      <span class="detail-total">{{ t('admin.maintenance.totalRows', { n: detailData.total }) }}</span>
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
