<template>
  <div class="app-container">
    <el-row :gutter="20">
      <el-col :span="4" :xs="24">
        <div class="head-container">
          <el-input
            v-model="deptName"
            placeholder="请输入部门名称"
            clearable
            prefix-icon="Search"
            style="margin-bottom: 20px"
          />
        </div>
        <div class="head-container">
          <el-tree
            ref="deptTreeRef"
            :data="deptOptions"
            :props="{ label: 'label', children: 'children' }"
            :expand-on-click-node="false"
            :filter-node-method="filterNode"
            node-key="id"
            highlight-current
            default-expand-all
            @node-click="handleNodeClick"
          />
        </div>
      </el-col>

      <el-col :span="20" :xs="24">
        <el-form :model="queryParams" ref="queryRef" :inline="true" v-show="showSearch" label-width="80px">
          <el-form-item label="名称" prop="name">
            <el-input
              v-model="queryParams.name"
              placeholder="请输入模型配置名称"
              clearable
              style="width: 220px"
              @keyup.enter="handleQuery"
            />
          </el-form-item>
          <el-form-item label="提供商" prop="provider">
            <el-select v-model="queryParams.provider" placeholder="请选择提供商" clearable style="width: 180px">
              <el-option v-for="item in providerOptions" :key="item.value" :label="item.label" :value="item.value" />
            </el-select>
          </el-form-item>
          <el-form-item label="状态" prop="isActive">
            <el-select v-model="queryParams.isActive" placeholder="请选择状态" clearable style="width: 160px">
              <el-option label="启用" :value="1" />
              <el-option label="停用" :value="0" />
            </el-select>
          </el-form-item>
          <el-form-item>
            <el-button type="primary" icon="Search" @click="handleQuery">搜索</el-button>
            <el-button icon="Refresh" @click="resetQuery">重置</el-button>
          </el-form-item>
        </el-form>

        <el-row :gutter="10" class="mb8">
          <el-col :span="1.5">
            <el-button type="primary" plain icon="Plus" @click="handleAdd">新增部门模型</el-button>
          </el-col>
          <el-col :span="1.5">
            <el-button type="success" plain icon="Edit" :disabled="single" @click="handleUpdate">修改</el-button>
          </el-col>
          <el-col :span="1.5">
            <el-button type="danger" plain icon="Delete" :disabled="multiple" @click="handleDelete">删除</el-button>
          </el-col>
          <right-toolbar v-model:showSearch="showSearch" @queryTable="getList"></right-toolbar>
        </el-row>

        <el-table v-loading="loading" :data="modelList" @selection-change="handleSelectionChange">
          <el-table-column type="selection" width="50" align="center" />
          <el-table-column label="名称" align="center" prop="name" min-width="150" :show-overflow-tooltip="true" />
          <el-table-column label="部门" align="center" prop="deptName" min-width="140" :show-overflow-tooltip="true">
            <template #default="scope">{{ scope.row.deptName || "-" }}</template>
          </el-table-column>
          <el-table-column label="提供商" align="center" prop="provider" width="120">
            <template #default="scope">
              <el-tag>{{ providerLabel(scope.row.provider) }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column label="模型" align="center" prop="model" min-width="160" :show-overflow-tooltip="true" />
          <el-table-column label="Base URL" align="center" prop="baseUrl" min-width="220" :show-overflow-tooltip="true" />
          <el-table-column label="API Key" align="center" min-width="120">
            <template #default="scope">
              <el-tag :type="scope.row.apiKeyHash ? 'success' : 'info'">
                {{ scope.row.apiKeyHash ? "已配置" : "未配置" }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="状态" align="center" prop="isActive" width="90">
            <template #default="scope">
              <el-tag :type="scope.row.isActive === 1 ? 'success' : 'info'">
                {{ scope.row.isActive === 1 ? "启用" : "停用" }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="创建时间" align="center" prop="createdAt" width="170">
            <template #default="scope">{{ parseTime(scope.row.createdAt) }}</template>
          </el-table-column>
          <el-table-column label="操作" align="center" width="140" class-name="small-padding fixed-width">
            <template #default="scope">
              <el-tooltip content="修改" placement="top">
                <el-button link type="primary" icon="Edit" @click="handleUpdate(scope.row)"></el-button>
              </el-tooltip>
              <el-tooltip content="删除" placement="top">
                <el-button link type="primary" icon="Delete" @click="handleDelete(scope.row)"></el-button>
              </el-tooltip>
            </template>
          </el-table-column>
        </el-table>

        <pagination
          v-show="total > 0"
          :total="total"
          v-model:page="queryParams.pageNum"
          v-model:limit="queryParams.pageSize"
          @pagination="getList"
        />
      </el-col>
    </el-row>

    <el-dialog :title="title" v-model="open" width="680px" append-to-body>
      <el-form :model="form" :rules="rules" ref="modelRef" label-width="100px">
        <el-row :gutter="16">
          <el-col :span="12">
            <el-form-item label="名称" prop="name">
              <el-input v-model="form.name" placeholder="例如：研发部 DeepSeek" maxlength="100" />
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="归属部门" prop="deptId">
              <el-popover placement="bottom-start" trigger="click" width="320">
                <template #reference>
                  <el-input v-model="selectedDeptLabel" placeholder="请选择部门" readonly />
                </template>
                <el-tree
                  :data="deptOptions"
                  :props="{ label: 'label', children: 'children' }"
                  node-key="id"
                  highlight-current
                  default-expand-all
                  @node-click="handleFormDeptClick"
                />
              </el-popover>
            </el-form-item>
          </el-col>
        </el-row>
        <el-row :gutter="16">
          <el-col :span="12">
            <el-form-item label="提供商" prop="provider">
              <el-select v-model="form.provider" placeholder="请选择提供商" style="width: 100%" @change="handleProviderChange">
                <el-option v-for="item in providerOptions" :key="item.value" :label="item.label" :value="item.value" />
              </el-select>
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="状态" prop="isActive">
              <el-switch v-model="form.isActive" :active-value="1" :inactive-value="0" />
            </el-form-item>
          </el-col>
        </el-row>
        <el-form-item label="模型" prop="model">
          <el-select
            v-model="form.model"
            filterable
            allow-create
            default-first-option
            placeholder="Select or enter a model"
            style="width: 100%"
          >
            <el-option v-for="item in modelOptions" :key="item" :label="item" :value="item" />
          </el-select>
        </el-form-item>
        <el-form-item label="Base URL" prop="baseUrl">
          <el-select
            v-model="form.baseUrl"
            filterable
            allow-create
            default-first-option
            placeholder="Select or enter a Base URL"
            style="width: 100%"
          >
            <el-option v-for="item in baseUrlOptions" :key="item" :label="item" :value="item" />
          </el-select>
        </el-form-item>
        <el-form-item label="API Key" prop="apiKeyHash">
          <el-input
            v-model="form.apiKeyHash"
            type="password"
            show-password
            placeholder="用于桌面端一键配置 Claude Code"
            maxlength="500"
          />
        </el-form-item>
        <el-alert
          type="info"
          :closable="false"
          show-icon
          title="父部门用户可查看本部门及所有子部门的启用模型；一个部门可以配置多个模型。"
        />
      </el-form>
      <template #footer>
        <div class="dialog-footer">
          <el-button type="primary" @click="submitForm">确定</el-button>
          <el-button @click="cancel">取消</el-button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<script setup name="TeamModel">
import { listTeamModel, getTeamModel, addTeamModel, updateTeamModel, delTeamModel } from "@/api/team/model"
import { deptTreeSelect } from "@/api/system/user"

const { proxy } = getCurrentInstance()

const modelList = ref([])
const open = ref(false)
const loading = ref(true)
const showSearch = ref(true)
const ids = ref([])
const single = ref(true)
const multiple = ref(true)
const total = ref(0)
const title = ref("")
const deptName = ref("")
const deptOptions = ref([])
const selectedDeptLabel = ref("")

const providerOptions = [
  { label: "OpenAI", value: "openai" },
  { label: "Anthropic", value: "anthropic" },
  { label: "DeepSeek", value: "deepseek" },
  { label: "阿里百炼", value: "bailian" },
  { label: "智谱 AI", value: "zhipu" },
  { label: "月之暗面", value: "moonshot" },
  { label: "MiniMax", value: "minimax" },
  { label: "本地模型", value: "local" },
  { label: "自定义", value: "custom" }
]

const providerPresets = {
  openai: {
    baseUrls: ["https://api.openai.com/v1"],
    models: ["gpt-5.5", "gpt-5.5-pro", "gpt-5.4", "gpt-5.4-pro", "gpt-5.1", "o3", "gpt-4.1"]
  },
  anthropic: {
    baseUrls: ["https://api.anthropic.com"],
    models: ["claude-opus-4-5", "claude-sonnet-4-5", "claude-haiku-4-5"]
  },
  deepseek: {
    baseUrls: ["https://api.deepseek.com", "https://api.deepseek.com/anthropic"],
    models: ["deepseek-v4-pro", "deepseek-v4-flash", "deepseek-chat", "deepseek-reasoner"]
  },
  bailian: {
    baseUrls: ["https://dashscope.aliyuncs.com/compatible-mode/v1"],
    models: ["qwen3.7-max", "qwen3.6-max-preview", "qwen3.6-plus", "qwen-plus", "qwen-max"]
  },
  zhipu: {
    baseUrls: ["https://open.bigmodel.cn/api/paas/v4", "https://open.bigmodel.cn/api/anthropic"],
    models: ["glm-5.1", "glm-5", "glm-4.7-flash", "glm-4.6"]
  },
  moonshot: {
    baseUrls: ["https://api.moonshot.cn/v1"],
    models: ["kimi-k2.6", "kimi-k2.5", "moonshot-v1-128k", "moonshot-v1-32k"]
  },
  minimax: {
    baseUrls: ["https://api.minimaxi.com/v1", "https://api.minimaxi.com/anthropic"],
    models: ["MiniMax-M2.7"]
  },
  local: {
    baseUrls: ["http://127.0.0.1:11434/v1", "http://127.0.0.1:1234/v1"],
    models: ["llama3.3", "qwen2.5", "deepseek-r1"]
  },
  custom: {
    baseUrls: [],
    models: []
  }
}

const activePreset = computed(() => providerPresets[form.value.provider] || providerPresets.custom)
const modelOptions = computed(() => activePreset.value.models)
const baseUrlOptions = computed(() => activePreset.value.baseUrls)

const data = reactive({
  form: {},
  queryParams: {
    pageNum: 1,
    pageSize: 10,
    name: undefined,
    provider: undefined,
    deptId: undefined,
    includeChildren: true,
    isActive: undefined
  },
  rules: {
    name: [{ required: true, message: "名称不能为空", trigger: "blur" }],
    provider: [{ required: true, message: "提供商不能为空", trigger: "change" }],
    model: [{ required: true, message: "模型不能为空", trigger: "blur" }],
    deptId: [{ required: true, message: "部门不能为空", trigger: "change" }]
  }
})

const { queryParams, form, rules } = toRefs(data)

function providerLabel(value) {
  return providerOptions.find(item => item.value === value)?.label || value
}

function handleProviderChange(provider) {
  const preset = providerPresets[provider] || providerPresets.custom
  form.value.baseUrl = preset.baseUrls[0]
  form.value.model = preset.models[0]
}

function filterNode(value, data) {
  if (!value) return true
  return data.label.indexOf(value) !== -1
}

watch(deptName, val => {
  proxy.$refs.deptTreeRef.filter(val)
})

function getDeptTree() {
  deptTreeSelect().then(response => {
    deptOptions.value = response.data || []
  })
}

function findDeptLabel(nodes, id) {
  if (!id) return ""
  for (const node of nodes || []) {
    if (node.id === id) return node.label
    const childLabel = findDeptLabel(node.children, id)
    if (childLabel) return childLabel
  }
  return ""
}

function getList() {
  loading.value = true
  listTeamModel(queryParams.value).then(res => {
    modelList.value = res.rows
    total.value = res.total
  }).catch(() => {
    modelList.value = []
    total.value = 0
  }).finally(() => {
    loading.value = false
  })
}

function handleNodeClick(data) {
  queryParams.value.deptId = data.id
  handleQuery()
}

function handleFormDeptClick(data) {
  form.value.deptId = data.id
  selectedDeptLabel.value = data.label
}

function handleQuery() {
  queryParams.value.pageNum = 1
  getList()
}

function resetQuery() {
  proxy.resetForm("queryRef")
  queryParams.value.deptId = undefined
  queryParams.value.includeChildren = true
  proxy.$refs.deptTreeRef.setCurrentKey(null)
  handleQuery()
}

function handleSelectionChange(selection) {
  ids.value = selection.map(item => item.id)
  single.value = selection.length !== 1
  multiple.value = !selection.length
}

function reset() {
  form.value = {
    id: undefined,
    name: undefined,
    provider: "deepseek",
    baseUrl: providerPresets.deepseek.baseUrls[0],
    model: providerPresets.deepseek.models[0],
    apiKeyHash: undefined,
    deptId: queryParams.value.deptId,
    isActive: 1
  }
  selectedDeptLabel.value = findDeptLabel(deptOptions.value, form.value.deptId)
  proxy.resetForm("modelRef")
}

function cancel() {
  open.value = false
  reset()
}

function handleAdd() {
  reset()
  open.value = true
  title.value = "新增部门模型"
}

function handleUpdate(row) {
  reset()
  const id = row.id || ids.value[0]
  getTeamModel(id).then(response => {
    form.value = response.data
    selectedDeptLabel.value = findDeptLabel(deptOptions.value, form.value.deptId)
    open.value = true
    title.value = "修改部门模型"
  })
}

function handleDelete(row) {
  const modelIds = row.id || ids.value.join(",")
  proxy.$modal.confirm('是否确认删除模型配置编号为 "' + modelIds + '" 的数据项？').then(function () {
    return delTeamModel(modelIds)
  }).then(() => {
    getList()
    proxy.$modal.msgSuccess("删除成功")
  }).catch(() => {})
}

function submitForm() {
  proxy.$refs.modelRef.validate(valid => {
    if (valid) {
      if (form.value.id !== undefined) {
        updateTeamModel(form.value).then(() => {
          proxy.$modal.msgSuccess("修改成功")
          open.value = false
          getList()
        })
      } else {
        addTeamModel(form.value).then(() => {
          proxy.$modal.msgSuccess("新增成功")
          open.value = false
          getList()
        })
      }
    }
  })
}

getDeptTree()
getList()
</script>
