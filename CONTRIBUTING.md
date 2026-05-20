# Contributing to Self Evolving Skills

感谢你考虑贡献！以下是参与本项目的方式。

## 环境准备

```bash
git clone https://github.com/xinbaizhe/SelfEvolvingSkills.git
cd SelfEvolvingSkills/frontend
npm install
npm run tauri:dev
```

## 技术栈

| 层 | 技术 |
|:---|:---|
| 桌面框架 | Tauri 2 |
| 前端 | Vue 3 + TypeScript + Vite + Element Plus |
| 后端 | Rust + rusqlite + reqwest + tokio |

## 提交规范

- 遵循 [Conventional Commits](https://www.conventionalcommits.org/) 格式
- 类型: `feat` `fix` `refactor` `docs` `test` `chore` `perf` `ci`
- 示例: `feat: add skill export to markdown`

## 代码风格

- 前端: Vue 3 Composition API + `<script setup lang="ts">`
- 后端: `cargo fmt` + `cargo clippy`
- 所有公开 API 使用 `ApiResponse<T>` 统一响应格式

## 分支策略

- `master` — 稳定发布分支
- `feat/xxx` — 功能开发
- `fix/xxx` — Bug 修复

## 隐私原则

- 所有数据处理默认在本地完成
- 不上传用户对话内容、路径信息
- 社区检索仅请求 GitHub 公开 API，不携带本地数据

## Issue / PR

- Bug 报告请包含复现步骤和系统环境
- 功能请求请先开 Issue 讨论
- PR 请关联对应 Issue
