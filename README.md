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

> 所有数据默认留在本机，不上传任何内容。

## 支持的数据源

| Claude Code | Codex | Hermes | Cursor | VSCode | CodeBuddy | TRAE | ZeeLinClaw |
|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| Skills · Agents · Sessions · Memory | Skills · Sessions | Skills · Sessions | Sessions | Sessions | Skills · Sessions | Skills · Sessions | Skills · Sessions |

## 核心流程：7 步进化管道

```
扫描发现 ──→ 参考检索 ──→ 聚类分析 ──→ 生成草稿 ──→ 智能优化 ──→ 质量评审 ──→ 差异推荐
(0-20%)     (20-32%)     (32-50%)     (50-62%)    (62-78%)     (78-90%)      (90-100%)
```

| # | 阶段 | 说明 |
|---|------|------|
| 1 | **扫描发现** | 检测已安装 AI 助手，扫描会话/Skills/Agents/Memories |
| 2 | **参考检索** | 从 GitHub 社区搜索相似 Skills 作为聚类参考 |
| 3 | **聚类分析** | 对历史会话做工作流聚类 + LLM 复核排序 |
| 4 | **生成草稿** | 纯本地模板生成 Skill 草稿（不调用 LLM） |
| 5 | **智能优化** | LLM 逐条优化草稿，注入社区参考 |
| 6 | **质量评审** | QA Agent 评审 + 6 项 guardrails（frontmatter 校验、名称唯一性、工具安全、可执行性等） |
| 7 | **差异推荐** | 对比社区 Skills，输出 replace / merge / keep / install 可操作建议 |

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

## 隐私

- 扫描、聚类、压缩均在本地执行，原始会话不上传
- 社区检索仅请求 GitHub 公开 API，不携带本地数据
- ZIP 导入含路径穿越和大小校验
- 无外部数据库依赖，无遥测采集

## License

MIT
