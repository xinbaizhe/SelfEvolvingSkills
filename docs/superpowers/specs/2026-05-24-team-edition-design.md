# 团队版设计文档

**日期**: 2026-05-24
**状态**: 已确认

---

## 一、整体架构

三层独立系统，通过 HTTP API 通信：

```
桌面客户端 (Tauri + Vue3)
  ├── 个人版 (无需登录): 本地扫描、进化管道、Skills 管理
  ├── 团队版 (登录后解锁): Skills 共享、协同进化、团队工作流
  └── Tauri Rust 层: SQLite + JWT管理 + API代理 (本地 or 远程)
        │ HTTP (JWT Bearer)
        ▼
Java 后台 API (Spring Boot / RuoYi-Vue3)
  ├── 用户认证 (Spring Security + JWT)
  ├── 组织管理 (部门/岗位/角色/RBAC)
  ├── 团队 Skills 服务 (共享、进化、统计)
  └── MySQL/PostgreSQL
        │
        ▼
Web 管理后台 (RuoYi-Vue3 Vue3 前端)
  ├── 用户管理、角色权限、系统监控
  ├── 组织架构、数据统计
  └── 模型配置管理
```

### 目录结构 (方案 A：同一仓库)

```
SelfEvolvingSkills/
├── desktop/                 ← 桌面端 (现在的 frontend/)
│   ├── src/                 (Vue3 前端)
│   ├── src-tauri/           (Rust)
│   └── package.json
├── server/                  ← Java Spring Boot 后端 (RuoYi 后端)
│   ├── ruoyi-admin/
│   ├── ruoyi-common/
│   ├── ruoyi-framework/
│   ├── ruoyi-system/
│   ├── ruoyi-team/          ← 新增模块：团队版业务
│   └── pom.xml
├── admin-ui/                ← Web 管理后台 Vue3 前端 (RuoYi 前端)
│   ├── src/
│   └── package.json
└── docs/
```

### 核心原则

- 个人版所有数据在本地 SQLite，不依赖网络
- 登录后桌面端获得两个数据源：本地 + 远程，前端透明合并
- Rust 层统一决策每个 API 请求走本地还是远程
- 网络不可用时个人版功能完全不受影响，团队数据用本地缓存

---

## 二、桌面端改造

### 2.1 侧边栏登录区域

在侧边栏底部（隐私声明下方）增加登录区域：

- **未登录状态**：显示 "登录团队版" 按钮
- **已登录状态**：显示用户名 + 退出/切换按钮，解锁 "团队空间" 菜单项

### 2.2 Vue 层新增

- **`useTeamStore`** (Pinia): `authenticated`、`username`、`orgName`、`serverUrl`
- **路由守卫**: 团队版路由需要 `authenticated = true`
- **API 层**: `src/api/team.ts` — 所有团队 API 走 Tauri invoke

### 2.3 Rust 层新增

- **`services/team_service.rs`**: HTTP 客户端，调用 Java 后台 API
- **Token 管理**: 写入 OS 凭据库 (Windows Credential Manager / macOS Keychain)，不在前端存储
- **新增 Tauri commands**: `login_team`、`logout_team`、`team_proxy`
- **API 路由**: 团队相关路径在现有 dispatch 中转发到 team_proxy

### 2.4 数据隔离

| 本地 SQLite (个人版) | Java MySQL (团队版) |
|---|---|
| skills (个人扫描) | team_skills (共享) |
| sessions (本地扫描) | team_evolutions |
| workflow_clusters | team_model_configs |
| agent 配置 | sys_user/sys_role/sys_dept |

---

## 三、Java 后台 API

### 3.1 认证 (桌面端用)

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/team/login` | 桌面端登录 (RuoYi /login 基础上新增 device 参数) |
| POST | `/api/team/logout` | 退出登录 |
| GET | `/api/team/profile` | 获取用户信息 + 组织 + 可用模型列表 |

### 3.2 团队 Skills (新增模块 ruoyi-team)

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/team/skills` | 列表 (搜索/分类/model/agent 筛选) |
| GET | `/api/team/skills/:id` | 详情 |
| POST | `/api/team/skills` | 上传分享 |
| PUT | `/api/team/skills/:id` | 更新 |
| DELETE | `/api/team/skills/:id` | 删除 |
| POST | `/api/team/skills/:id/install` | 安装记录 |
| GET | `/api/team/skills/stats` | 统计数据 |

### 3.3 协同进化

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/team/evolutions` | 进化列表 |
| POST | `/api/team/evolutions` | 提交进化建议 |
| GET | `/api/team/evolutions/:id` | 详情 |
| POST | `/api/team/evolutions/:id/approve` | 审批 |

### 3.4 安全

- JWT: Access Token 2h + Refresh Token 7d
- 组织数据隔离: 按 `dept_id` 过滤
- API 限流: 登录 5次/min, 普通 100次/min
- 密码传输: RSA 公钥加密后发送

---

## 四、数据库设计 (新增表)

### team_skills

```sql
CREATE TABLE team_skills (
    id              BIGINT AUTO_INCREMENT PRIMARY KEY,
    name            VARCHAR(200) NOT NULL,
    description     TEXT,
    category        VARCHAR(100),
    source_type     VARCHAR(50)  DEFAULT 'user',
    origin_agent    VARCHAR(50),
    body_md         MEDIUMTEXT NOT NULL,
    author_id       BIGINT NOT NULL,
    dept_id         BIGINT,
    compatible_models TEXT,          -- JSON: ["claude-sonnet-4-6", "*"]
    compatible_agents TEXT,          -- JSON: ["claude-code", "cursor"]
    usage_count     INT DEFAULT 0,
    avg_score       DECIMAL(3,2) DEFAULT 0,
    status          VARCHAR(20) DEFAULT 'published',
    version         INT DEFAULT 1,
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at      DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);
```

### team_skill_installs

```sql
CREATE TABLE team_skill_installs (
    id          BIGINT AUTO_INCREMENT PRIMARY KEY,
    skill_id    BIGINT NOT NULL,
    user_id     BIGINT NOT NULL,
    agent_id    VARCHAR(50),
    model_id    BIGINT,                  -- 关联 team_model_configs
    installed_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uk_skill_user_agent_model (skill_id, user_id, agent_id, model_id)
);
```

### team_evolutions

```sql
CREATE TABLE team_evolutions (
    id              BIGINT AUTO_INCREMENT PRIMARY KEY,
    skill_id        BIGINT NOT NULL,
    proposer_id     BIGINT NOT NULL,
    previous_version TEXT,
    proposed_change MEDIUMTEXT NOT NULL,
    reason          TEXT,
    status          VARCHAR(20) DEFAULT 'pending',
    reviewer_id     BIGINT,
    review_comment  TEXT,
    reviewed_at     DATETIME,
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### team_model_configs

```sql
CREATE TABLE team_model_configs (
    id          BIGINT AUTO_INCREMENT PRIMARY KEY,
    name        VARCHAR(100) NOT NULL,
    provider    VARCHAR(50)  NOT NULL,
    base_url    VARCHAR(500),
    model       VARCHAR(100) NOT NULL,
    api_key_hash VARCHAR(500),
    dept_id     BIGINT,
    created_by  BIGINT,
    is_active   TINYINT(1) DEFAULT 1,
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

---

## 五、多模型 + 多 Agent 架构

### 5.1 模式对比

| | 个人版 | 团队版 |
|---|---|---|
| 模型 | 1 个 LLM 配置 | M 个模型 (按部门/用户) |
| Agent | N 个 Agent | N 个 Agent |
| Skill 匹配 | 所有 Agent 共享 | 可按模型+Agent 组合绑定 |

### 5.2 Skills 工作台筛选

- 模型筛选器: 全部模型 / 部门模型 / 指定模型
- Agent 筛选器: 全部 Agent / 指定 Agent
- Skill 卡片显示当前筛选组合的兼容性和效果评分
- 安装时选择目标模型+Agent 对

---

## 六、登录认证流程

1. 用户点击 "登录团队版" → 弹窗输入服务器地址 + 用户名 + 密码
2. Rust 层 `login_team`: ping 校验 → 获取 RSA 公钥 → 加密密码 → POST /api/team/login
3. JWT 写入 OS 凭据库，用户信息返回前端
4. 前端 `useTeamStore` 更新状态，解锁团队菜单，拉取团队数据
5. Token 刷新: 401 时 Rust 层自动用 refresh_token 换新，对前端透明
6. 离线模式: 缓存数据只读，修改暂存队列，网络恢复后提交

---

## 七、实施顺序

| 阶段 | 内容 | 估时 |
|------|------|------|
| Phase 1 | 部署 RuoYi-Vue3 骨架 (server + admin-ui)，数据库初始化 | 搭建 |
| Phase 2 | 桌面端登录按钮 + Rust 层 Token 管理 + 登录弹窗 | 登录 |
| Phase 3 | Java 后台 team_skills CRUD API | 核心 |
| Phase 4 | 桌面端团队 Skills 列表 + 详情 + 分享功能 | 前端 |
| Phase 5 | 多模型配置 + Skills 工作台筛选升级 | 模型 |
| Phase 6 | 协同进化 API + 桌面端反馈提交 | 进化 |
| Phase 7 | 离线缓存 + 消息队列 + 容错处理 | 鲁棒性 |
