<div align="center">

<img src="frontend/src-tauri/icons/128x128@2x.png" width="96" height="96" alt="logo" />

# Self Evolving Skills

**从重复工作流中沉淀可复用的本地 Skills**

[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-0d9488?style=flat-square)](.)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-ffc131?style=flat-square&logo=tauri)](https://tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3.5-4fc08d?style=flat-square&logo=vue.js)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-edition2021-dea584?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE.txt)

</div>

---

## 这是什么

Self Evolving Skills 是一款**本地优先**的桌面应用。它会扫描你本机安装的 AI 编程工具，收集已有的 Skills、Agents、会话历史，然后通过 **7 步进化管道**从真实工作流中自动发现高频重复任务，生成可安装的 Skill 草稿。

> 核心价值：**把你和 AI 协作的真实经验，自动沉淀为可复用的 Skill，24小时不间断。**

> 所有数据默认留在本机，不上传任何内容。

## 24小时持续运行

系统通过三层机制实现无人值守的知识沉淀：

```
┌────────────────────────────────────────────────────┐
│                  24h 持续运行层                       │
├──────────────────┬────────────────┬────────────────┤
│  文件监听器       │  定时扫描       │  进化管道        │
│  (File Watchers) │  (Periodic)    │  (Pipeline)     │
├──────────────────┼────────────────┼────────────────┤
│ 实时检测 Skills   │ 全量会话扫描    │ 7步自动进化      │
│ Agents/Sessions  │ 结构化压缩      │ 多Agent协作      │
│ 变化即触发        │ 增量更新        │ 草稿评审修正     │
└──────────────────┴────────────────┴────────────────┘
```

- **文件监听器**：通过 `notify` 监听 9 种 AI 工具目录的文件变更（Create/Modify/Remove），实时 upsert 到 SQLite WAL
- **会话压缩**：长对话 JSONL 先本地压缩（提取目标、工具调用、错误、结果、关键上下文），再聚类分析，原始数据不上传
- **数据生命周期**：实时采集 → 结构化压缩 → 聚类分析 → 草稿生成 → 安装使用 → 效果追踪 → 迭代进化

## 支持的数据源

| Claude Code | Codex | Hermes | OpenClaw | Cursor | VSCode | CodeBuddy | TRAE | ZeeLinClaw |
|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| Skills · Agents · Sessions · Memory | Skills · Sessions | Skills · Sessions | Skills · Agents · Sessions | Sessions | Sessions | Skills · Sessions | Skills · Sessions | Skills · Sessions |

## 大模型配置

在"系统配置"页面可配置 LLM 连接，用于聚类复核、草稿优化和质量评审。支持 **OpenAI Chat Completions** 和 **Anthropic Messages** 两种 API 格式，切换服务商时自动匹配对应端点。

| 服务商 | 推荐模型 | API 格式 |
|:---|:---|:---:|
| OpenAI | gpt-5.5 / gpt-5.5-pro / gpt-5.4 | OpenAI |
| DeepSeek | deepseek-v4-pro / deepseek-v4-flash | OpenAI + Anthropic |
| 阿里百炼 | qwen3.7-max / qwen3.6-plus | OpenAI |
| 智谱 AI | glm-5.1 / glm-5 | OpenAI + Anthropic |
| 月之暗面 | kimi-k2.6 / kimi-k2.5 | OpenAI |
| MiniMax | MiniMax-M2.7 | OpenAI + Anthropic |

> 未配置大模型时，进化管道自动使用本地回退逻辑，不发起网络请求。

## 核心流程：7 步进化管道

```
扫描发现 ──→ 参考检索 ──→ 聚类分析 ──→ 生成草稿 ──→ 智能优化 ──→ 质量评审 ──→ 差异推荐
(0-20%)     (20-32%)     (32-50%)     (50-62%)    (62-78%)     (78-90%)      (90-100%)
```

| # | 阶段 | 进度 | 说明 |
|---|------|:---:|------|
| 1 | **扫描发现** | 0-20% | 遍历 9 个 Agent 目录：Skills（YAML+Markdown 解析）、Agents（工具/模型/配置）、Sessions（JSONL+FTS5 索引）、Memories（类型归类） |
| 2 | **参考检索** | 20-32% | 并行 GitHub API 搜索 + LLM 推荐社区 Skills，加权评分（stars×0.4 + relevance×0.4 + quality×0.2），写入本地缓存 |
| 3 | **聚类分析** | 32-50% | TF-IDF 关键词提取 → 领域关键词匹配 → Subject-Action-Artifact 三元组命名 → 可选 LLM 复核排序 |
| 4 | **生成草稿** | 50-62% | 纯本地模板渲染，输出标准 YAML frontmatter + Markdown body（概述/场景/信号/步骤/验证/安全），不调用 LLM |
| 5 | **智能优化** | 62-78% | **三 Agent 协作**：Judge（评审打分）→ Critic（找盲点）→ Fixer（综合输出），含 8 项自查清单 |
| 6 | **质量评审** | 78-90% | QA Agent 5 维度评审（安全/性能/功能/写法/改进），评分 <75 自动反馈修正（最多 3 轮），保留最高分版本 |
| 7 | **差异推荐** | 90-100% | 本地草稿 vs 社区 Skills 并行对比，输出 replace / merge / keep / install 可操作建议 |

### Step 5 三 Agent 协作细节

```
原草稿
  → Judge（评审专家）：找出问题 + 建议 + 打分
  → Critic（批判者）：检验 Judge 评审是否合理，找盲点
  → Fixer（修复专家）：综合两者意见，输出最终草稿 + 8项自查
```

### Step 6 QA 评审维度

| 维度 | 检查内容 |
|------|---------|
| safety | 安全注意事项、工具权限控制、"不要使用时机"边界 |
| performance | 步骤数量（3-8）、执行效率、缓存策略 |
| functionality | 输入输出明确性、适用场景描述、触发信号 |
| writing | YAML frontmatter 完整性、kebab-case 命名、描述清晰度 |
| improvements | 具体改进建议，可操作的优化方向 |

## Skills 工作台

| Tab | 功能 |
|:---|:---|
| **进化管道** | 启动 7 步进化，实时进度环 + 阶段状态条 |
| **推荐** | 聚类工作流列表，按来源筛选（频率分析 / 大模型复核 / 手动进化） |
| **草稿** | 查看和编辑生成的 Skill 草稿，支持保存、删除、安装 |
| **社区对比** | 本地草稿 vs GitHub 社区 Skills 并行对比 |
| **已生成** | 可安装的已审核草稿，一键安装到目标 Agent |
| **操作记录** | 历史进化管道执行记录，分页查看每步状态 |

## 技术栈

| 层 | 技术 |
|:---|:---|
| 桌面框架 | **Tauri 2** — 轻量原生体验 |
| 前端 | **Vue 3** + TypeScript + Vite + Element Plus + Pinia |
| 后端 | **Rust** + rusqlite + reqwest + tokio |
| 存储 | **SQLite WAL** — 本地高性能读写 |
| 自动更新 | `tauri-plugin-updater` + GitHub Releases |
| CI/CD | GitHub Actions (Win / Mac / Linux 并行构建) |

## 快速开始

```bash
git clone https://github.com/xinbaizhe/SelfEvolvingSkills.git
cd SelfEvolvingSkills/frontend
npm install
npm run tauri:dev
```

## 打包

```bash
npm run tauri:build
```

| 平台 | 产物 |
|:---|---|
| Windows | `.exe` (NSIS 安装包) / `.msi` |
| macOS | `.dmg` |
| Linux | `.deb` / `.AppImage` |

> 原生打包需在对应平台执行。推送到 GitHub 后，CI 自动在三平台并行构建，支持自动更新。

## API 速查

所有请求通过 Tauri `invoke("api_request", ...)` 本地调用，统一响应 `{ success, data, error }`。

| 模块 | 端点 | 说明 |
|:---|:---|:---|
| 进化管道 | `POST /evolution/start` | 启动 7 步进化管道 |
| | `GET /evolution/status` | 获取当前管道进度和阶段状态 |
| | `POST /evolution/reset` | 重置卡住的管道 |
| | `GET /evolution/history` | 历史执行记录（分页） |
| Skills | `GET /skills` | 分页列表（支持搜索、按 Agent 筛选） |
| | `PUT /skills/:name` | 编辑 Skill |
| | `DELETE /skills/:name` | 删除 Skill |
| | `POST /skills/import` | 导入 Markdown / JSON / ZIP |
| | `POST /skills/:name/evolve` | 手动进化 |
| 工作流 | `GET /workflows` | 推荐工作流列表（含聚类草稿） |
| | `PUT /workflows/:id` | 更新草稿正文 |
| | `DELETE /workflows/:id` | 删除草稿 |
| | `POST /workflows/:id/install` | 安装草稿到目标 Agent |
| | `GET /workflows/install-targets` | 可安装的 Agent 目标列表 |
| 社区 | `GET /community/search` | 搜索 GitHub 社区 Skills |
| | `POST /community/install` | 下载安装社区 Skill |
| 系统 | `GET /system/monitor` | 系统资源监控（CPU / 内存 / 磁盘） |
| | `GET /system/database` | 数据库信息 |
| 管理 | `GET /admin/system` | 系统概览（Skills / Agents / Sessions 统计） |
| | `GET /admin/config/llm` | 获取大模型配置 |
| | `PUT /admin/config/llm` | 保存大模型配置 |
| | `POST /admin/config/llm/test` | 测试大模型连接 |

## 同类对比

| 维度 | Self Evolving Skills | Cursor Rules | GitHub Copilot Instructions | 手动编写 |
|------|---------------------|-------------|---------------------------|---------|
| **生成方式** | 自动从历史会话聚类 | 手动编写 | 手动编写 | 纯手写 |
| **数据来源** | 9种AI工具真实会话 | 仅 Cursor | 仅 Copilot | 无数据驱动 |
| **持续进化** | 24h文件监听+管道 | 无 | 无 | 手动维护 |
| **社区参考** | 自动搜索GitHub+对比 | 无 | 无 | 手动搜索 |
| **质量保证** | 3 Agent评审+修正循环 | 无 | 无 | 靠自己 |
| **离线能力** | 全本地，无需网络 | 本地文件 | 本地文件 | 本地文件 |
| **多Agent支持** | 9 种 | 仅Cursor | 仅Copilot | 跨平台手动 |

| 效率指标 | 手动模式 | Self Evolving Skills |
|---------|---------|---------------------|
| 发现重复工作流 | 依赖记忆，易遗漏 | 自动聚类，全量扫描 |
| Skill 生成时间 | 30-60分钟/个 | 2-5分钟/个（全自动） |
| 知识沉淀率 | <20%（遗忘为主） | >80%（自动归档） |

## 常见问题

**Q: 数据安全吗？会不会上传我的会话？**

100% 本地。扫描、聚类、压缩均在本地 SQLite 执行，原始会话不上传。LLM 调用仅在用户主动配置 API Key 后才生效。GitHub 搜索仅请求公开 API，不携带本地数据。可以断网使用。

**Q: 不配置大模型能用吗？**

能。进化管道自动回退到本地模式：聚类用 TF-IDF 算法，草稿用模板生成，评审和对比跳过或用规则引擎替代。启用 LLM 后质量会更好，但本地模式也能产出可用草稿。

**Q: 生成的 Skill 质量如何保证？**

三重保证：① 三 Agent 协作（Judge→Critic→Fixer）生成草稿；② QA Agent 5 维度评审 + 反馈修正循环（<75分自动重生成，最多3轮）；③ 6 项 Guardrails（frontmatter 校验、名称唯一性、工具安全、可执行性等）。

**Q: 多久运行一次进化管道？**

建议每周 1-2 次，或会话积累到 50+ 时运行。文件监听器持续运行，增量更新不影响管道频率。通常 2-5 分钟完成。

**Q: 支持哪些 AI 编程工具？**

9 种：Claude Code、Codex、Hermes、OpenClaw、Cursor、VSCode、CodeBuddy、TRAE、ZeeLinClaw。每个工具的目录位置不同，系统自动检测。

## 未来路线图

| 阶段 | 内容 | 状态 |
|------|------|:---:|
| Phase 1 | 个人版基础功能 + 7步进化管道 | ✅ |
| Phase 2 | 多Agent A/B变体 + 效果追踪 | ✅ |
| Phase 3 | 反馈修正循环 + 评分阈值优化 | ✅ |
| Phase 4 | 团队版 Java后台 + Web管理 | 📋 |
| Phase 5 | 协同进化 + Skills市场 | 📋 |
| Phase 6 | 定时自动进化 + 通知推送 | 📋 |

## 隐私

- 扫描、聚类、压缩均在本地执行，原始会话不上传
- LLM 调用仅在用户主动启用并配置服务商后生效，可随时关闭回退到本地模式
- 社区检索仅请求 GitHub 公开 API，不携带本地数据
- ZIP 导入含路径穿越和大小校验
- 无外部数据库依赖，无遥测采集

## License

MIT
