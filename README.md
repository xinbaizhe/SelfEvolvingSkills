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

Self Evolving Skills 是一款**本地优先**的桌面应用。它会扫描你本机安装的 AI 编程工具，收集已有的 Skills、Agents、会话历史，然后从真实工作流中**自动发现高频重复任务**，生成可安装的 Skill 草稿。

> 所有数据默认留在本机，不上传任何内容。

## 支持的数据源

| Claude Code | Codex | Hermes | Cursor | VSCode | CodeBuddy | TRAE | ZeeLinClaw |
|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| Skills · Agents · Sessions · Memory | Skills · Sessions | Skills · Sessions | Sessions | Sessions | Skills · Sessions | Skills · Sessions | Skills · Sessions |

## 核心流程

```mermaid
flowchart LR
  Scan[扫描本机 Agent 数据] --> Cluster[会话压缩 & 工作流聚类]
  Cluster --> Draft[生成 Skill 草稿]
  Draft --> Compare[GitHub 社区对比参考]
  Compare --> Review[人工审核编辑]
  Review --> Install[一键安装到目标 Agent]
```

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
| 扫描 | `POST /scan` | 启动全量扫描 |
| Skills | `GET /skills` | 分页列表（支持搜索、按 Agent 筛选） |
| | `PUT /skills/:name` | 编辑 Skill |
| | `DELETE /skills/:name` | 删除 Skill |
| | `POST /skills/import` | 导入 Markdown / JSON / ZIP |
| | `POST /skills/:name/evolve` | 手动进化 |
| 工作流 | `GET /workflows` | 聚类草稿列表 |
| | `POST /workflows/cluster` | 触发聚类 |
| | `POST /workflows/:id/install` | 安装草稿到 Agent |
| 进化 | `POST /evolution/start` | 启动进化管道 |
| 社区 | `GET /community/search` | GitHub 搜索 Skills |
| | `POST /community/install` | 下载安装社区 Skill |

## 隐私

- 扫描、聚类、压缩均在本地执行，原始会话不上传
- 社区检索仅请求 GitHub 公开 API，不携带本地数据
- ZIP 导入含路径穿越和大小校验
- 无外部数据库依赖，无遥测采集

## License

MIT
