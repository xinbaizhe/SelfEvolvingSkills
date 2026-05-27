<div align="center">

<img src="frontend/src-tauri/icons/128x128@2x.png" width="96" height="96" alt="logo" />

# Self Evolving Skills

**从重复工作流中沉淀可复用的本地 Skills**

[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-0d9488?style=flat-square)](.)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-ffc131?style=flat-square&logo=tauri)](https://tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3.5-4fc08d?style=flat-square&logo=vue.js)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-edition2021-dea584?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![Spring Boot](https://img.shields.io/badge/Spring_Boot-4.0-6db33f?style=flat-square&logo=springboot)](https://spring.io)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE.txt)

</div>

---

## 项目概览

Self Evolving Skills 包含三个子系统：

| 子系统 | 目录 | 技术栈 | 用途 |
|:---|:---|:---|:---|
| **桌面应用** | `frontend/` | Tauri 2 + Vue 3 + Rust | 个人版本地桌面应用 |
| **团队后台** | `server/` | Spring Boot 4 + MyBatis + MySQL | 团队版 API 服务 |
| **管理界面** | `admin-ui/` | Vue 3 + Element Plus + Vite | 团队版 Web 管理后台 |

---

## 系统架构

```
┌──────────────────────────────────────────────────┐
│                   个人版（本地）                     │
│  ┌──────────┐  ┌──────────┐  ┌────────────────┐  │
│  │ Vue 3 前端│  │ Rust 后端 │  │ SQLite (WAL)   │  │
│  │ Element+ │  │  Tauri 2  │  │ 本地文件系统     │  │
│  └──────────┘  └──────────┘  └────────────────┘  │
│       桌面应用 (Tauri 2) → npm run tauri:dev       │
└──────────────────────────────────────────────────┘
                        ↕ API
┌──────────────────────────────────────────────────┐
│                  团队版（网络）                      │
│  ┌──────────┐  ┌──────────────┐  ┌────────────┐  │
│  │ Admin UI │  │ Spring Boot  │  │ MySQL/Redis │  │
│  │ Vue 3    │  │ RuoYi 框架    │  │             │  │
│  └──────────┘  └──────────────┘  └────────────┘  │
│   npm run dev    mvn spring-boot:run               │
└──────────────────────────────────────────────────┘
```

---

## 环境要求

### 个人版（桌面应用）

| 工具 | 版本要求 | 说明 |
|:---|:---|:---|
| Node.js | ≥18 | 前端构建 + Tauri CLI |
| Rust | latest stable | 后端编译 |
| Windows | Windows 10+ | 需安装 [Microsoft Visual C++ Build Tools](https://visualstudio.microsoft.com/zh-hans/visual-cpp-build-tools/) |
| macOS | macOS 11+ | 需安装 Xcode Command Line Tools |
| Linux | Ubuntu 20.04+ | 需安装 `libwebkit2gtk-4.1-dev` 等系统依赖 |

### 团队版（Java 后台 + Web 管理界面）

| 工具 | 版本要求 | 说明 |
|:---|:---|:---|
| JDK | 17 | Java 运行环境 |
| Maven | 3.8+ | 项目构建 |
| MySQL | 8.0+ | 数据库 |
| Redis | 6.0+ | 缓存 |
| Node.js | ≥18 | Admin UI 前端 |

---

## 个人版（桌面应用）启动

### 1. 安装前端依赖

```bash
cd frontend
npm install
```

### 2. 启动开发服务器

```bash
npm run tauri:dev
```

首次运行会自动编译 Rust 后端（约 2-5 分钟），后续热更新秒级生效。

### 3. 生产构建

```bash
npm run tauri:build
```

| 平台 | 产物 |
|:---|---|
| Windows | `.exe` (NSIS 安装包) / `.msi` |
| macOS | `.dmg` |
| Linux | `.deb` / `.AppImage` |

> 原生打包需在对应平台执行。推送到 GitHub 后，CI 自动在三平台并行构建，支持自动更新。

---

## 团队版启动

团队版由 **Java 后台**（server/）和 **Web 管理界面**（admin-ui/）两部分组成，需分别启动。

### 第一步：初始化数据库

1. 创建 MySQL 数据库：
```sql
CREATE DATABASE ry_self_evolving DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci;
```

2. 按顺序执行 SQL 脚本：
```bash
mysql -u root -p ry_self_evolving < server/sql/ry_20260417.sql    # RuoYi 基础表
mysql -u root -p ry_self_evolving < server/sql/quartz.sql          # 定时任务表
mysql -u root -p ry_self_evolving < server/sql/team_tables.sql     # 团队版 Skills 表
```

### 第二步：配置后台

编辑 `server/ruoyi-admin/src/main/resources/application.yml`：

```yaml
spring:
  datasource:
    druid:
      master:
        url: jdbc:mysql://localhost:3306/ry_self_evolving?useUnicode=true&characterEncoding=utf8
        username: root
        password: your_password          # ← 修改为你的数据库密码
  data:
    redis:
      host: localhost
      port: 6379
      password:                          # ← 如果 Redis 有密码，在此填写
```

其他可配置项：
- `server.port`：后台服务端口（默认 8080）
- `token.secret`：JWT 签名密钥（生产环境请修改）
- `ruoyi.profile`：文件上传路径（Windows 示例 `D:/ruoyi/uploadPath`，Linux 示例 `/home/ruoyi/uploadPath`）

### 第三步：启动后台服务

```bash
cd server

# 开发模式（热部署）
mvn spring-boot:run

# 或先打包再运行
mvn clean package -DskipTests
java -jar ruoyi-admin/target/ruoyi-admin.jar
```

启动成功后访问：
- API 文档：http://localhost:8080/swagger-ui.html
- 默认管理员：admin / admin123

### 第四步：启动管理界面

```bash
cd admin-ui
npm install
npm run dev
```

管理界面运行在 `http://localhost:80`，已配置反向代理将 `/dev-api` 转发到后台 `http://localhost:8080`。

若后台端口不是 8080，编辑 `admin-ui/vite.config.js` 中的 proxy target。

环境变量配置文件：
- `admin-ui/.env.development` — 开发环境
- `admin-ui/.env.production` — 生产环境
- `admin-ui/.env.staging` — 预发布环境

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

## 团队版 Skills 管理

团队版提供 Web 管理后台，支持多部门协同管理 Skills：

| 功能 | 说明 |
|:---|:---|
| **Skills 管理** | 按部门/岗位/角色筛选 Skills，支持 CRUD 和搜索 |
| **部门组织** | 树形部门结构，各部门独立管理 Skills |
| **角色与岗位** | 关联系统角色（`sys_role`）和岗位（`sys_post`）进行筛选 |
| **模型配置** | 按部门配置 LLM 模型，用于进化管道 |
| **进化记录** | 追踪每次进化管道的执行状态和结果 |
| **认证登录** | 基于 Spring Security + JWT 的用户认证 |

管理界面入口：[admin-ui/](admin-ui/)，启动后访问 `http://localhost:80`，默认账号 `admin / admin123`。

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

### 团队版技术栈

| 层 | 技术 |
|:---|:---|
| 后台框架 | **Spring Boot 4** (RuoYi-Vue 3.9.2) |
| 持久层 | MyBatis + Druid 连接池 |
| 缓存 | Redis + Spring Cache |
| 接口文档 | Springdoc (Swagger 3) |
| 前端 | Vue 3 + Element Plus + Vite + Pinia |
| 数据库 | MySQL 8.0 |
| 团队模块 | `server/ruoyi-team/` — Skills 管理、模型配置、进化记录 |

## 快速开始

```bash
git clone https://github.com/xinbaizhe/SelfEvolvingSkills.git
cd SelfEvolvingSkills/frontend
npm install
npm run tauri:dev
```

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
| 团队 | `POST /api/team/login` | 团队版登录 |
| | `GET /api/team/profile` | 当前用户信息和可用模型 |
| | `GET /api/team/deptTree` | 部门树形结构 |
| | `GET /api/team/roles` | 岗位（角色）列表 |
| | `GET /api/team/posts` | 职务列表 |
| | `GET /api/team/skills/list` | 团队 Skills 列表（支持部门/岗位/角色筛选） |
| | `POST /api/team/skills` | 新增团队 Skill |
| | `PUT /api/team/skills` | 编辑团队 Skill |
| | `DELETE /api/team/skills/:ids` | 删除团队 Skill |

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
| Phase 4 | 团队版 Java后台 + Web管理 | ✅ |
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
