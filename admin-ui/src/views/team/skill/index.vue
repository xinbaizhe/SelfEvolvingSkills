<template>
  <div class="app-container">
    <el-row :gutter="20">
      <!--部门树-->
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
            :data="deptOptions"
            :props="{ label: 'label', children: 'children' }"
            :expand-on-click-node="false"
            :filter-node-method="filterNode"
            ref="deptTreeRef"
            node-key="id"
            highlight-current
            default-expand-all
            @node-click="handleNodeClick"
          />
        </div>
      </el-col>
      <!--技能数据-->
      <el-col :span="20" :xs="24">
        <el-form :model="queryParams" ref="queryRef" :inline="true" v-show="showSearch" label-width="68px">
          <el-form-item label="技能名称" prop="name">
            <el-input
              v-model="queryParams.name"
              placeholder="请输入技能名称"
              clearable
              style="width: 240px"
              @keyup.enter="handleQuery"
            />
          </el-form-item>
          <el-form-item label="分类" prop="category">
            <el-select
              v-model="queryParams.category"
              placeholder="技能分类"
              clearable
              style="width: 240px"
            >
              <el-option label="编程开发" value="coding" />
              <el-option label="日报数据处理" value="daily_report" />
              <el-option label="日常办公" value="office" />
              <el-option label="数据分析" value="data" />
              <el-option label="测试调试" value="testing" />
              <el-option label="运维部署" value="devops" />
              <el-option label="文档编写" value="docs" />
              <el-option label="设计创意" value="design" />
              <el-option label="其他" value="other" />
            </el-select>
          </el-form-item>
          <el-form-item label="来源类型" prop="sourceType">
            <el-select
              v-model="queryParams.sourceType"
              placeholder="来源类型"
              clearable
              style="width: 240px"
            >
              <el-option label="Claude Code" value="claude_code" />
              <el-option label="Cursor" value="cursor" />
              <el-option label="GitHub Copilot" value="copilot" />
              <el-option label="Windsurf" value="windsurf" />
              <el-option label="Cline" value="cline" />
              <el-option label="Continue" value="continue" />
              <el-option label="Aider" value="aider" />
              <el-option label="CodeBuddy" value="codebuddy" />
              <el-option label="其他工具" value="other_tool" />
            </el-select>
          </el-form-item>
          <el-form-item label="角色" prop="roleId">
            <el-select
              v-model="queryParams.roleId"
              placeholder="按创建者角色筛选"
              clearable
              style="width: 240px"
            >
              <el-option
                v-for="item in roleOptions"
                :key="item.roleId"
                :label="item.roleName"
                :value="item.roleId"
              />
            </el-select>
          </el-form-item>
          <el-form-item label="状态" prop="status">
            <el-select
              v-model="queryParams.status"
              placeholder="技能状态"
              clearable
              style="width: 240px"
            >
              <el-option label="已发布" value="published" />
              <el-option label="草稿" value="draft" />
              <el-option label="审核中" value="review" />
              <el-option label="已归档" value="archived" />
            </el-select>
          </el-form-item>
          <el-form-item>
            <el-button type="primary" icon="Search" @click="handleQuery">搜索</el-button>
            <el-button icon="Refresh" @click="resetQuery">重置</el-button>
          </el-form-item>
        </el-form>

        <el-row :gutter="10" class="mb8">
          <el-col :span="1.5">
            <el-button
              type="primary"
              plain
              icon="Plus"
              @click="handleAdd"
            >新增</el-button>
          </el-col>
          <el-col :span="1.5">
            <el-button
              type="success"
              plain
              icon="Edit"
              :disabled="single"
              @click="handleUpdate"
            >修改</el-button>
          </el-col>
          <el-col :span="1.5">
            <el-button
              type="danger"
              plain
              icon="Delete"
              :disabled="multiple"
              @click="handleDelete"
            >删除</el-button>
          </el-col>
          <right-toolbar v-model:showSearch="showSearch" @queryTable="getList"></right-toolbar>
        </el-row>

        <el-table v-loading="loading" :data="skillList" @selection-change="handleSelectionChange">
          <el-table-column type="selection" width="50" align="center" />
          <el-table-column label="技能名称" align="center" prop="name" :show-overflow-tooltip="true" />
          <el-table-column label="分类" align="center" prop="category" width="100" />
          <el-table-column label="描述" align="center" prop="description" :show-overflow-tooltip="true" />
          <el-table-column label="来源" align="center" prop="sourceType" width="100">
            <template #default="scope">
              <el-tag v-if="scope.row.sourceType === 'claude_code'">Claude Code</el-tag>
              <el-tag v-else-if="scope.row.sourceType === 'cursor'" type="success">Cursor</el-tag>
              <el-tag v-else-if="scope.row.sourceType === 'copilot'" type="info">GitHub Copilot</el-tag>
              <el-tag v-else-if="scope.row.sourceType === 'windsurf'">Windsurf</el-tag>
              <el-tag v-else-if="scope.row.sourceType === 'cline'" type="warning">Cline</el-tag>
              <el-tag v-else-if="scope.row.sourceType === 'continue'" type="success">Continue</el-tag>
              <el-tag v-else-if="scope.row.sourceType === 'aider'" type="info">Aider</el-tag>
              <el-tag v-else-if="scope.row.sourceType === 'codebuddy'">CodeBuddy</el-tag>
              <el-tag v-else-if="scope.row.sourceType === 'other_tool'" type="danger">其他工具</el-tag>
              <span v-else>{{ scope.row.sourceType }}</span>
            </template>
          </el-table-column>
          <el-table-column label="使用次数" align="center" prop="usageCount" width="100" />
          <el-table-column label="平均评分" align="center" prop="avgScore" width="100">
            <template #default="scope">
              <span v-if="scope.row.avgScore">{{ scope.row.avgScore }}</span>
              <span v-else>-</span>
            </template>
          </el-table-column>
          <el-table-column label="版本" align="center" prop="version" width="70" />
          <el-table-column label="状态" align="center" prop="status" width="90">
            <template #default="scope">
              <el-tag v-if="scope.row.status === 'published'" type="success">已发布</el-tag>
              <el-tag v-else-if="scope.row.status === 'draft'" type="info">草稿</el-tag>
              <el-tag v-else-if="scope.row.status === 'review'" type="warning">审核中</el-tag>
              <el-tag v-else-if="scope.row.status === 'archived'" type="danger">已归档</el-tag>
              <span v-else>{{ scope.row.status }}</span>
            </template>
          </el-table-column>
          <el-table-column label="操作" align="center" width="150" class-name="small-padding fixed-width">
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

    <!-- 添加或修改团队技能对话框 -->
    <el-dialog :title="title" v-model="open" width="700px" append-to-body>
      <el-form :model="form" :rules="rules" ref="skillRef" label-width="100px">
        <el-row>
          <el-col :span="12">
            <el-form-item label="技能名称" prop="name">
              <el-input v-model="form.name" placeholder="请输入技能名称" maxlength="100" />
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="分类" prop="category">
              <el-select v-model="form.category" placeholder="请选择分类" style="width: 100%">
                <el-option label="编程开发" value="coding" />
                <el-option label="日报数据处理" value="daily_report" />
                <el-option label="日常办公" value="office" />
                <el-option label="数据分析" value="data" />
                <el-option label="测试调试" value="testing" />
                <el-option label="运维部署" value="devops" />
                <el-option label="文档编写" value="docs" />
                <el-option label="设计创意" value="design" />
                <el-option label="其他" value="other" />
              </el-select>
            </el-form-item>
          </el-col>
        </el-row>
        <el-row>
          <el-col :span="12">
            <el-form-item label="来源类型" prop="sourceType">
              <el-select v-model="form.sourceType" placeholder="请选择来源类型" style="width: 100%">
                <el-option label="Claude Code" value="claude_code" />
                <el-option label="Cursor" value="cursor" />
                <el-option label="GitHub Copilot" value="copilot" />
                <el-option label="Windsurf" value="windsurf" />
                <el-option label="Cline" value="cline" />
                <el-option label="Continue" value="continue" />
                <el-option label="Aider" value="aider" />
                <el-option label="CodeBuddy" value="codebuddy" />
                <el-option label="其他工具" value="other_tool" />
              </el-select>
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="状态" prop="status">
              <el-select v-model="form.status" placeholder="请选择状态" style="width: 100%">
                <el-option label="已发布" value="published" />
                <el-option label="草稿" value="draft" />
                <el-option label="审核中" value="review" />
                <el-option label="已归档" value="archived" />
              </el-select>
            </el-form-item>
          </el-col>
        </el-row>
        <el-row>
          <el-col :span="24">
            <el-form-item label="描述" prop="description">
              <el-input v-model="form.description" type="textarea" placeholder="请输入技能描述" maxlength="500" />
            </el-form-item>
          </el-col>
        </el-row>
        <el-row>
          <el-col :span="12">
            <el-form-item label="兼容模型" prop="compatibleModels">
              <el-input v-model="form.compatibleModels" placeholder="如: GPT-4,Claude" maxlength="200" />
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="兼容智能体" prop="compatibleAgents">
              <el-input v-model="form.compatibleAgents" placeholder="如: Claude Code,Cursor" maxlength="200" />
            </el-form-item>
          </el-col>
        </el-row>
        <el-row>
          <el-col :span="24">
            <el-form-item label="技能内容" prop="bodyMd">
              <el-input v-model="form.bodyMd" type="textarea" :rows="6" placeholder="请输入技能内容（Markdown格式）" />
            </el-form-item>
          </el-col>
        </el-row>
      </el-form>
      <template #footer>
        <div class="dialog-footer">
          <el-button type="primary" @click="submitForm">确 定</el-button>
          <el-button @click="cancel">取 消</el-button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<script setup name="TeamSkill">
import { listTeamSkill, getTeamSkill, addTeamSkill, updateTeamSkill, delTeamSkill } from "@/api/team/skill"
import { deptTreeSelect } from "@/api/system/user"
import { listRole } from "@/api/system/role"

const { proxy } = getCurrentInstance()

const skillList = ref([])
const open = ref(false)
const loading = ref(true)
const showSearch = ref(true)
const ids = ref([])
const single = ref(true)
const multiple = ref(true)
const total = ref(0)
const title = ref("")
const deptName = ref("")
const deptOptions = ref(undefined)
const roleOptions = ref([])

const data = reactive({
  form: {},
  queryParams: {
    pageNum: 1,
    pageSize: 10,
    name: undefined,
    category: undefined,
    sourceType: undefined,
    status: undefined,
    deptId: undefined,
    roleId: undefined
  },
  rules: {
    name: [{ required: true, message: "技能名称不能为空", trigger: "blur" }],
    category: [{ required: true, message: "分类不能为空", trigger: "change" }],
    description: [{ required: true, message: "描述不能为空", trigger: "blur" }],
    bodyMd: [{ required: true, message: "技能内容不能为空", trigger: "blur" }]
  }
})

const { queryParams, form, rules } = toRefs(data)

/** 过滤部门树节点 */
const filterNode = (value, data) => {
  if (!value) return true
  return data.label.indexOf(value) !== -1
}

watch(deptName, val => {
  proxy.$refs["deptTreeRef"].filter(val)
})

/** 查询部门下拉树结构 */
function getDeptTree() {
  deptTreeSelect().then(response => {
    deptOptions.value = response.data
  })
}

/** 查询角色列表 */
function getRoleList() {
  listRole({ pageNum: 1, pageSize: 100 }).then(response => {
    roleOptions.value = response.rows
  })
}

/** 查询技能列表 */
function getList() {
  loading.value = true
  listTeamSkill(queryParams.value).then(res => {
    loading.value = false
    skillList.value = res.rows
    total.value = res.total
  })
}

/** 节点单击事件 */
function handleNodeClick(data) {
  queryParams.value.deptId = data.id
  handleQuery()
}

/** 搜索按钮操作 */
function handleQuery() {
  queryParams.value.pageNum = 1
  getList()
}

/** 重置按钮操作 */
function resetQuery() {
  proxy.resetForm("queryRef")
  queryParams.value.deptId = undefined
  queryParams.value.roleId = undefined
  proxy.$refs.deptTreeRef.setCurrentKey(null)
  handleQuery()
}

/** 删除按钮操作 */
function handleDelete(row) {
  const skillIds = row.id || ids.value.join(",")
  proxy.$modal.confirm('是否确认删除技能编号为"' + skillIds + '"的数据项？').then(function () {
    return delTeamSkill(skillIds)
  }).then(() => {
    getList()
    proxy.$modal.msgSuccess("删除成功")
  }).catch(() => {})
}

/** 选择条数 */
function handleSelectionChange(selection) {
  ids.value = selection.map(item => item.id)
  single.value = selection.length != 1
  multiple.value = !selection.length
}

/** 重置操作表单 */
function reset() {
  form.value = {
    id: undefined,
    name: undefined,
    category: undefined,
    description: undefined,
    sourceType: undefined,
    originAgent: undefined,
    bodyMd: undefined,
    compatibleModels: undefined,
    compatibleAgents: undefined,
    status: "published"
  }
  proxy.resetForm("skillRef")
}

/** 取消按钮 */
function cancel() {
  open.value = false
  reset()
}

/** 新增按钮操作 */
function handleAdd() {
  reset()
  open.value = true
  title.value = "添加团队技能"
}

/** 修改按钮操作 */
function handleUpdate(row) {
  reset()
  const id = row.id || ids.value[0]
  getTeamSkill(id).then(response => {
    form.value = response.data
    open.value = true
    title.value = "修改团队技能"
  })
}

/** 提交按钮 */
function submitForm() {
  proxy.$refs["skillRef"].validate(valid => {
    if (valid) {
      if (form.value.id != undefined) {
        updateTeamSkill(form.value).then(() => {
          proxy.$modal.msgSuccess("修改成功")
          open.value = false
          getList()
        })
      } else {
        addTeamSkill(form.value).then(() => {
          proxy.$modal.msgSuccess("新增成功")
          open.value = false
          getList()
        })
      }
    }
  })
}

getDeptTree()
getRoleList()
getList()
</script>
