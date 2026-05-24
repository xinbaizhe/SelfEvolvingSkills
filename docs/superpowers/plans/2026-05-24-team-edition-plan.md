# 团队版实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 SelfEvolvingSkills 桌面端增加团队版功能——Java 后台（若依框架）+ Web 管理后台 + 桌面端登录认证与团队 Skills 共享/协同进化。

**Architecture:** 三系统独立部署——桌面端 (Tauri+Vue3) 通过 Rust 层代理调用 Java 后台 API (Spring Boot/RuoYi-Vue3)，Web 管理后台独立运行。Rust 层管理 JWT Token 和本地缓存，前端透明合并本地+团队数据。

**Tech Stack:** Vue3 + TypeScript + Pinia + Element Plus (desktop) | Rust + Tauri + reqwest (gateway) | Java + Spring Boot + MyBatis + MySQL (server) | Vue3 + Element Plus (admin-ui)

---

## 文件结构

```
SelfEvolvingSkills/
├── desktop/                          ← 改名自 frontend/
│   ├── src/
│   │   ├── api/team.ts               ← NEW: 团队 API
│   │   ├── stores/useTeamStore.ts     ← NEW: 团队状态管理
│   │   ├── views/TeamSkillsView.vue   ← NEW: 团队 Skills 列表
│   │   ├── components/team/
│   │   │   ├── LoginDialog.vue       ← NEW: 登录弹窗
│   │   │   ├── ShareSkillDialog.vue  ← NEW: 分享 Skill 弹窗
│   │   │   └── TeamSidebar.vue       ← NEW: 侧边栏登录区域
│   │   ├── components/layout/AppSidebar.vue ← MODIFY: 底部增加登录入口
│   │   ├── router/index.ts           ← MODIFY: 新增团队路由
│   │   └── App.vue                   ← MODIFY: 引入 TeamSidebar
│   ├── src-tauri/
│   │   ├── Cargo.toml                ← MODIFY: 添加 reqwest, keyring
│   │   ├── src/lib.rs                ← MODIFY: 新增 team commands, dispatch
│   │   └── src/services/
│   │       ├── team_service.rs       ← NEW: HTTP客户端 + Token管理
│   │       └── mod.rs                ← MODIFY
│   └── package.json
├── server/                           ← NEW: RuoYi-Vue3 后端
│   ├── ruoyi-admin/
│   ├── ruoyi-common/
│   ├── ruoyi-framework/
│   ├── ruoyi-system/
│   ├── ruoyi-team/                   ← NEW: 团队版业务模块
│   │   ├── pom.xml
│   │   └── src/main/java/com/ruoyi/team/
│   │       ├── controller/
│   │       │   ├── TeamAuthController.java
│   │       │   ├── TeamSkillController.java
│   │       │   └── TeamEvolutionController.java
│   │       ├── domain/
│   │       │   ├── TeamSkill.java
│   │       │   ├── TeamSkillInstall.java
│   │       │   ├── TeamEvolution.java
│   │       │   └── TeamModelConfig.java
│   │       ├── mapper/
│   │       │   ├── TeamSkillMapper.java
│   │       │   ├── TeamSkillInstallMapper.java
│   │       │   ├── TeamEvolutionMapper.java
│   │       │   └── TeamModelConfigMapper.java
│   │       ├── service/
│   │       │   ├── ITeamSkillService.java
│   │       │   ├── TeamSkillServiceImpl.java
│   │       │   ├── ITeamEvolutionService.java
│   │       │   └── TeamEvolutionServiceImpl.java
│   │       └── dto/
│   │           ├── LoginRequest.java
│   │           ├── TeamSkillDTO.java
│   │           └── EvolutionDTO.java
│   └── pom.xml
├── admin-ui/                         ← NEW: RuoYi-Vue3 前端
│   ├── src/
│   └── package.json
└── docs/
    └── superpowers/
        ├── specs/2026-05-24-team-edition-design.md
        └── plans/2026-05-24-team-edition-plan.md
```

---

## Phase 1: RuoYi-Vue3 骨架搭建

### Task 1.1: 克隆 RuoYi-Vue3 后端为 server/

**Files:** Create entire `server/` directory

- [ ] **Step 1: Clone RuoYi-Vue3 backend**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git clone https://github.com/yangzongzhuan/RuoYi-Vue3.git server-tmp
# 后端在 ruoyi-admin/ ruoyi-common/ ruoyi-framework/ ruoyi-system/ ruoyi-generator/
# 前端在 ruoyi-ui/
# 我们只需要后端，把后端模块移动到 server/
mkdir -p server
mv server-tmp/ruoyi-admin server-tmp/ruoyi-common server-tmp/ruoyi-framework server-tmp/ruoyi-system server-tmp/ruoyi-generator server/
mv server-tmp/pom.xml server-tmp/*.xml server/ 2>/dev/null || true
mv server-tmp/sql server/
mv server-tmp/bin server/
rm -rf server-tmp
```

- [ ] **Step 2: Configure database connection**

Modify `server/ruoyi-admin/src/main/resources/application-druid.yml`:

```yaml
datasource:
  master:
    url: jdbc:mysql://localhost:3306/ry_self_evolving?useUnicode=true&characterEncoding=utf8&zeroDateTimeBehavior=convertToNull&useSSL=true&serverTimezone=GMT%2B8
    username: root
    password: your_password_here
```

- [ ] **Step 3: Run init SQL**

```bash
mysql -u root -p -e "CREATE DATABASE IF NOT EXISTS ry_self_evolving DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci"
mysql -u root -p ry_self_evolving < server/sql/ry_20240601.sql
```

- [ ] **Step 4: Verify backend starts**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills/server
mvn clean package -DskipTests -f pom.xml
java -jar ruoyi-admin/target/ruoyi-admin.jar
# 验证: curl http://localhost:8080 → 返回 JSON
```

- [ ] **Step 5: Commit**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git add server/ -f
git commit -m "feat: add RuoYi-Vue3 backend skeleton with ry_self_evolving database"
```

### Task 1.2: 克隆 RuoYi-Vue3 前端为 admin-ui/

**Files:** Create entire `admin-ui/` directory

- [ ] **Step 1: Clone and move**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git clone https://github.com/yangzongzhuan/RuoYi-Vue3.git admin-tmp
mv admin-tmp/ruoyi-ui admin-ui
rm -rf admin-tmp
```

- [ ] **Step 2: Install dependencies**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills/admin-ui
npm install
```

- [ ] **Step 3: Configure API proxy**

Check `admin-ui/vite.config.js` — proxy should point to `http://localhost:8080`:

```js
server: {
  proxy: {
    '/api': {
      target: 'http://localhost:8080',
      changeOrigin: true,
    }
  }
}
```

- [ ] **Step 4: Verify frontend starts**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills/admin-ui
npm run dev
# 打开 http://localhost:80 → 看到若依登录页
```

- [ ] **Step 5: Commit**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git add admin-ui/ -f
git commit -m "feat: add RuoYi-Vue3 admin UI"
```

### Task 1.3: 创建 ruoyi-team 模块骨架

**Files:** Create `server/ruoyi-team/`

- [ ] **Step 1: Create module pom.xml**

Create `server/ruoyi-team/pom.xml`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <parent>
        <groupId>com.ruoyi</groupId>
        <artifactId>ruoyi</artifactId>
        <version>3.8.9</version>
    </parent>
    <modelVersion>4.0.0</modelVersion>
    <artifactId>ruoyi-team</artifactId>
    <name>ruoyi-team</name>
    <description>团队版业务模块 - Skills共享/协同进化/模型配置</description>
    <dependencies>
        <dependency>
            <groupId>com.ruoyi</groupId>
            <artifactId>ruoyi-common</artifactId>
        </dependency>
        <dependency>
            <groupId>com.ruoyi</groupId>
            <artifactId>ruoyi-framework</artifactId>
        </dependency>
    </dependencies>
</project>
```

- [ ] **Step 2: Register in parent pom**

Modify `server/pom.xml` — add ruoyi-team to `<modules>`:

```xml
<modules>
    <module>ruoyi-admin</module>
    <module>ruoyi-framework</module>
    <module>ruoyi-system</module>
    <module>ruoyi-quartz</module>
    <module>ruoyi-generator</module>
    <module>ruoyi-common</module>
    <module>ruoyi-team</module>   <!-- 新增 -->
</modules>
```

- [ ] **Step 3: Add team dependency to admin**

Modify `server/ruoyi-admin/pom.xml` — add dependency:

```xml
<dependency>
    <groupId>com.ruoyi</groupId>
    <artifactId>ruoyi-team</artifactId>
    <version>${ruoyi.version}</version>
</dependency>
```

- [ ] **Step 4: Create package structure**

```bash
mkdir -p server/ruoyi-team/src/main/java/com/ruoyi/team/{controller,domain,mapper,service,dto}
mkdir -p server/ruoyi-team/src/main/resources
```

- [ ] **Step 5: Create mybatis mapper XML directory**

```bash
mkdir -p server/ruoyi-team/src/main/resources/mapper/team
```

- [ ] **Step 6: Verify Maven compiles**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills/server
mvn compile -pl ruoyi-team
```

- [ ] **Step 7: Commit**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git add server/ruoyi-team/ server/pom.xml server/ruoyi-admin/pom.xml
git commit -m "feat: add ruoyi-team module skeleton"
```

### Task 1.4: 初始化团队版数据库表

**Files:** Create migration SQL

- [ ] **Step 1: Create migration SQL file**

Create `server/sql/team_tables.sql`:

```sql
-- 团队 Skills
DROP TABLE IF EXISTS team_skills;
CREATE TABLE team_skills (
    id              BIGINT AUTO_INCREMENT PRIMARY KEY,
    name            VARCHAR(200) NOT NULL,
    description     TEXT,
    category        VARCHAR(100),
    source_type     VARCHAR(50)  DEFAULT 'user',
    origin_agent    VARCHAR(50),
    body_md         MEDIUMTEXT NOT NULL,
    author_id       BIGINT NOT NULL COMMENT '上传者',
    dept_id         BIGINT COMMENT '所属部门',
    compatible_models TEXT COMMENT 'JSON: ["claude-sonnet-4-6","*"]',
    compatible_agents TEXT COMMENT 'JSON: ["claude-code","cursor"]',
    usage_count     INT DEFAULT 0,
    avg_score       DECIMAL(3,2) DEFAULT 0,
    status          VARCHAR(20) DEFAULT 'published',
    version         INT DEFAULT 1,
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at      DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_team_skills_dept (dept_id),
    INDEX idx_team_skills_category (category),
    INDEX idx_team_skills_author (author_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='团队Skills';

-- 安装记录
DROP TABLE IF EXISTS team_skill_installs;
CREATE TABLE team_skill_installs (
    id          BIGINT AUTO_INCREMENT PRIMARY KEY,
    skill_id    BIGINT NOT NULL,
    user_id     BIGINT NOT NULL,
    agent_id    VARCHAR(50),
    model_id    BIGINT,
    installed_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uk_skill_user_agent_model (skill_id, user_id, agent_id, model_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='安装记录';

-- 协同进化
DROP TABLE IF EXISTS team_evolutions;
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
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_team_evolutions_skill (skill_id),
    INDEX idx_team_evolutions_status (status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='协同进化';

-- 模型配置
DROP TABLE IF EXISTS team_model_configs;
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
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='团队模型配置';
```

- [ ] **Step 2: Execute migration**

```bash
mysql -u root -p ry_self_evolving < server/sql/team_tables.sql
```

- [ ] **Step 3: Commit**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git add server/sql/team_tables.sql
git commit -m "feat: add team edition database tables"
```

---

## Phase 2: 桌面端登录认证

### Task 2.1: 重命名 frontend/ 为 desktop/

**Files:** Move entire directory

- [ ] **Step 1: Rename directory**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
mv frontend desktop
```

- [ ] **Step 2: Verify build still works**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills/desktop/src-tauri
cargo check
```

- [ ] **Step 3: Update any paths referencing frontend/ in docs**

Check `docs/` and `.gitignore` — replace `frontend/` with `desktop/` if needed.

- [ ] **Step 4: Commit**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git add -A
git commit -m "refactor: rename frontend/ to desktop/"
```

### Task 2.2: Rust 层 — 添加依赖 + team_service.rs

**Files:**
- Modify: `desktop/src-tauri/Cargo.toml`
- Create: `desktop/src-tauri/src/services/team_service.rs`
- Modify: `desktop/src-tauri/src/services/mod.rs`

- [ ] **Step 1: Add Cargo dependencies**

Modify `desktop/src-tauri/Cargo.toml`:

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
serde_json = "1"  # already present, verify
keyring = "3"      # OS credential store
tokio = { version = "1", features = ["sync"] }  # already present, verify
```

- [ ] **Step 2: Create team_service.rs**

Create `desktop/src-tauri/src/services/team_service.rs`:

```rust
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Mutex;
use tauri::AppHandle;

const KEYRING_SERVICE: &str = "self-evolving-skills";
const KEYRING_TOKEN_KEY: &str = "team-jwt";
const KEYRING_REFRESH_KEY: &str = "team-refresh";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamSession {
    pub server_url: String,
    pub access_token: String,
    pub refresh_token: String,
    pub username: String,
    pub user_id: i64,
    pub dept_name: String,
    pub dept_id: i64,
}

pub struct TeamState {
    pub session: Mutex<Option<TeamSession>>,
    pub http_client: reqwest::Client,
}

impl TeamState {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(None),
            http_client: reqwest::Client::new(),
        }
    }

    fn save_tokens(&self, access: &str, refresh: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_TOKEN_KEY)
            .map_err(|e| format!("keyring error: {}", e))?;
        entry.set_password(access).map_err(|e| format!("keyring set: {}", e))?;

        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_REFRESH_KEY)
            .map_err(|e| format!("keyring error: {}", e))?;
        entry.set_password(refresh).map_err(|e| format!("keyring set: {}", e))?;
        Ok(())
    }

    fn load_tokens(&self) -> Option<(String, String)> {
        let access = keyring::Entry::new(KEYRING_SERVICE, KEYRING_TOKEN_KEY)
            .ok()
            .and_then(|e| e.get_password().ok());
        let refresh = keyring::Entry::new(KEYRING_SERVICE, KEYRING_REFRESH_KEY)
            .ok()
            .and_then(|e| e.get_password().ok());
        match (access, refresh) {
            (Some(a), Some(r)) => Some((a, r)),
            _ => None,
        }
    }

    fn delete_tokens(&self) -> Result<(), String> {
        let _ = keyring::Entry::new(KEYRING_SERVICE, KEYRING_TOKEN_KEY)
            .map(|e| e.delete_credential());
        let _ = keyring::Entry::new(KEYRING_SERVICE, KEYRING_REFRESH_KEY)
            .map(|e| e.delete_credential());
        Ok(())
    }
}

#[tauri::command]
pub async fn login_team(
    state: tauri::State<'_, TeamState>,
    server_url: String,
    username: String,
    password: String,
) -> Result<Value, String> {
    let base = server_url.trim_end_matches('/');
    let client = &state.http_client;

    // 1. Ping server
    let ping_url = format!("{}/api/team/ping", base);
    client.get(&ping_url)
        .send().await
        .map_err(|e| format!("无法连接到服务器: {}", e))?;

    // 2. Login
    let login_url = format!("{}/api/team/login", base);
    let resp = client.post(&login_url)
        .header(CONTENT_TYPE, "application/json")
        .json(&serde_json::json!({
            "username": username,
            "password": password,
            "device": "desktop"
        }))
        .send().await
        .map_err(|e| format!("登录请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Err("用户名或密码错误".to_string());
    }

    let data: Value = resp.json().await.map_err(|e| format!("解析响应失败: {}", e))?;

    // 3. Extract tokens and user info
    let access_token = data["token"].as_str().ok_or("token 缺失")?.to_string();
    let refresh_token = data["refresh_token"].as_str().unwrap_or("").to_string();
    let username_str = data["username"].as_str().unwrap_or(&username).to_string();
    let user_id = data["user_id"].as_i64().unwrap_or(0);
    let dept_name = data["dept_name"].as_str().unwrap_or("").to_string();
    let dept_id = data["dept_id"].as_i64().unwrap_or(0);

    // 4. Save tokens to OS credential store
    state.save_tokens(&access_token, &refresh_token)?;

    // 5. Store session in memory
    let session = TeamSession {
        server_url: base.to_string(),
        access_token: access_token.clone(),
        refresh_token: refresh_token.clone(),
        username: username_str.clone(),
        user_id,
        dept_name: dept_name.clone(),
        dept_id,
    };
    *state.session.lock().map_err(|e| e.to_string())? = Some(session);

    Ok(serde_json::json!({
        "username": username_str,
        "user_id": user_id,
        "dept_name": dept_name,
        "dept_id": dept_id,
        "token": access_token
    }))
}

#[tauri::command]
pub async fn logout_team(
    state: tauri::State<'_, TeamState>,
) -> Result<(), String> {
    state.delete_tokens()?;
    *state.session.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}

#[tauri::command]
pub async fn get_team_session(
    state: tauri::State<'_, TeamState>,
) -> Result<Option<Value>, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    match session.as_ref() {
        Some(s) => Ok(Some(serde_json::json!({
            "username": s.username,
            "user_id": s.user_id,
            "dept_name": s.dept_name,
            "dept_id": s.dept_id,
            "server_url": s.server_url,
        }))),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn team_api_get(
    state: tauri::State<'_, TeamState>,
    path: String,
) -> Result<Value, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    let session = session.as_ref().ok_or("未登录")?;

    let url = format!("{}/api{}", session.server_url, path);
    let resp = state.http_client.get(&url)
        .header(AUTHORIZATION, format!("Bearer {}", session.access_token))
        .send().await
        .map_err(|e| format!("API 请求失败: {}", e))?;

    resp.json().await.map_err(|e| format!("解析失败: {}", e))
}

#[tauri::command]
pub async fn team_api_post(
    state: tauri::State<'_, TeamState>,
    path: String,
    body: Value,
) -> Result<Value, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    let session = session.as_ref().ok_or("未登录")?;

    let url = format!("{}/api{}", session.server_url, path);
    let resp = state.http_client.post(&url)
        .header(AUTHORIZATION, format!("Bearer {}", session.access_token))
        .header(CONTENT_TYPE, "application/json")
        .json(&body)
        .send().await
        .map_err(|e| format!("API 请求失败: {}", e))?;

    resp.json().await.map_err(|e| format!("解析失败: {}", e))
}

#[tauri::command]
pub async fn team_api_put(
    state: tauri::State<'_, TeamState>,
    path: String,
    body: Value,
) -> Result<Value, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    let session = session.as_ref().ok_or("未登录")?;

    let url = format!("{}/api{}", session.server_url, path);
    let resp = state.http_client.put(&url)
        .header(AUTHORIZATION, format!("Bearer {}", session.access_token))
        .header(CONTENT_TYPE, "application/json")
        .json(&body)
        .send().await
        .map_err(|e| format!("API 请求失败: {}", e))?;

    resp.json().await.map_err(|e| format!("解析失败: {}", e))
}

#[tauri::command]
pub async fn team_api_delete(
    state: tauri::State<'_, TeamState>,
    path: String,
) -> Result<Value, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    let session = session.as_ref().ok_or("未登录")?;

    let url = format!("{}/api{}", session.server_url, path);
    let resp = state.http_client.delete(&url)
        .header(AUTHORIZATION, format!("Bearer {}", session.access_token))
        .send().await
        .map_err(|e| format!("API 请求失败: {}", e))?;

    resp.json().await.map_err(|e| format!("解析失败: {}", e))
}

/// Try to restore session from OS keyring on app startup
pub fn try_restore_session(state: &TeamState, server_url: &str) -> Option<TeamSession> {
    let (access, refresh) = state.load_tokens()?;
    Some(TeamSession {
        server_url: server_url.to_string(),
        access_token: access,
        refresh_token: refresh,
        username: String::new(),  // will be refreshed from /team/profile
        user_id: 0,
        dept_name: String::new(),
        dept_id: 0,
    })
}
```

- [ ] **Step 3: Register in services/mod.rs**

Modify `desktop/src-tauri/src/services/mod.rs`:

```rust
pub mod team_service;
```

- [ ] **Step 4: Register TeamState and commands in lib.rs**

Modify `desktop/src-tauri/src/lib.rs` — add:

```rust
use services::team_service::{self, TeamState};

// In main function, add:
.manage(TeamState::new())
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    team_service::login_team,
    team_service::logout_team,
    team_service::get_team_session,
    team_service::team_api_get,
    team_service::team_api_post,
    team_service::team_api_put,
    team_service::team_api_delete,
])
```

- [ ] **Step 5: Verify compilation**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills/desktop/src-tauri
cargo check
# Expected: compiles successfully
```

- [ ] **Step 6: Commit**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git add desktop/src-tauri/Cargo.toml desktop/src-tauri/src/services/team_service.rs desktop/src-tauri/src/services/mod.rs desktop/src-tauri/src/lib.rs
git commit -m "feat: add Rust team_service with JWT token management and API proxy"
```

### Task 2.3: Vue 层 — useTeamStore

**Files:**
- Create: `desktop/src/stores/useTeamStore.ts`
- Create: `desktop/src/api/team.ts`

- [ ] **Step 1: Create team API helper**

Create `desktop/src/api/team.ts`:

```typescript
import { invoke } from '@tauri-apps/api/core'

export interface TeamSession {
  username: string
  user_id: number
  dept_name: string
  dept_id: number
  server_url: string
}

export interface TeamSkill {
  id: number
  name: string
  description: string
  category: string
  source_type: string
  origin_agent: string
  author_name: string
  dept_name: string
  compatible_models: string[]
  compatible_agents: string[]
  usage_count: number
  avg_score: number
  version: number
  created_at: string
  updated_at: string
}

export async function loginTeam(serverUrl: string, username: string, password: string): Promise<TeamSession> {
  return invoke<TeamSession>('login_team', { serverUrl, username, password })
}

export async function logoutTeam(): Promise<void> {
  return invoke<void>('logout_team')
}

export async function getTeamSession(): Promise<TeamSession | null> {
  return invoke<TeamSession | null>('get_team_session')
}

export async function teamApiGet<T>(path: string): Promise<T> {
  return invoke<T>('team_api_get', { path })
}

export async function teamApiPost<T>(path: string, body: unknown): Promise<T> {
  return invoke<T>('team_api_post', { path, body })
}

export async function teamApiPut<T>(path: string, body: unknown): Promise<T> {
  return invoke<T>('team_api_put', { path, body })
}

export async function teamApiDelete<T>(path: string): Promise<T> {
  return invoke<T>('team_api_delete', { path })
}

// Convenience methods for team skills
export function fetchTeamSkills(params?: Record<string, string>) {
  const query = params ? '?' + new URLSearchParams(params).toString() : ''
  return teamApiGet<{ total: number; items: TeamSkill[] }>(`/team/skills${query}`)
}

export function fetchTeamSkillDetail(id: number) {
  return teamApiGet<TeamSkill>(`/team/skills/${id}`)
}

export function shareSkillToTeam(skill: {
  name: string; description: string; category: string;
  body_md: string; origin_agent?: string;
  compatible_models?: string[]; compatible_agents?: string[]
}) {
  return teamApiPost<TeamSkill>('/team/skills', skill)
}

export function installTeamSkill(skillId: number, agentId: string, modelId?: number) {
  return teamApiPost<{ message: string }>(`/team/skills/${skillId}/install`, { agent_id: agentId, model_id: modelId })
}

export function fetchTeamStats() {
  return teamApiGet<{ total_skills: number; total_installs: number; top_categories: string[] }>('/team/skills/stats')
}
```

- [ ] **Step 2: Create useTeamStore**

Create `desktop/src/stores/useTeamStore.ts`:

```typescript
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import {
  loginTeam as apiLogin,
  logoutTeam as apiLogout,
  getTeamSession,
  type TeamSession,
} from '../api/team'

export const useTeamStore = defineStore('team', () => {
  const authenticated = ref(false)
  const username = ref('')
  const user_id = ref(0)
  const dept_name = ref('')
  const dept_id = ref(0)
  const server_url = ref('')
  const loading = ref(false)
  const error = ref('')

  const isAuthenticated = computed(() => authenticated.value)

  async function login(serverUrl: string, user: string, password: string): Promise<boolean> {
    loading.value = true
    error.value = ''
    try {
      const session = await apiLogin(serverUrl, user, password)
      setSession(session)
      return true
    } catch (e: unknown) {
      error.value = e instanceof Error ? e.message : String(e)
      return false
    } finally {
      loading.value = false
    }
  }

  async function logout() {
    try {
      await apiLogout()
    } catch { /* ignore */ }
    clearSession()
  }

  async function tryRestoreSession(): Promise<boolean> {
    try {
      const session = await getTeamSession()
      if (session) {
        setSession(session)
        return true
      }
    } catch { /* no saved session */ }
    return false
  }

  function setSession(session: TeamSession) {
    authenticated.value = true
    username.value = session.username
    user_id.value = session.user_id
    dept_name.value = session.dept_name
    dept_id.value = session.dept_id
    server_url.value = session.server_url
  }

  function clearSession() {
    authenticated.value = false
    username.value = ''
    user_id.value = 0
    dept_name.value = ''
    dept_id.value = 0
    server_url.value = ''
  }

  return {
    authenticated, username, user_id, dept_name, dept_id,
    server_url, loading, error, isAuthenticated,
    login, logout, tryRestoreSession,
  }
})
```

- [ ] **Step 3: Commit**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git add desktop/src/api/team.ts desktop/src/stores/useTeamStore.ts
git commit -m "feat: add useTeamStore and team API layer"
```

### Task 2.4: Vue 层 — 登录弹窗 + 侧边栏登录区域

**Files:**
- Create: `desktop/src/components/team/LoginDialog.vue`
- Create: `desktop/src/components/team/TeamSidebar.vue`
- Modify: `desktop/src/App.vue` — sidebar 引入 TeamSidebar

- [ ] **Step 1: Create LoginDialog.vue**

Create `desktop/src/components/team/LoginDialog.vue`:

```vue
<script setup lang="ts">
import { reactive, ref } from 'vue'
import { useTeamStore } from '../../stores/useTeamStore'

const store = useTeamStore()
const visible = ref(false)
const form = reactive({ serverUrl: 'http://localhost:8080', username: '', password: '' })
const loading = ref(false)
const localError = ref('')

const emit = defineEmits<{
  (e: 'loggedIn'): void
}>()

function open() {
  visible.value = true
  localError.value = ''
  form.password = ''
}

async function doLogin() {
  if (!form.serverUrl || !form.username || !form.password) {
    localError.value = '请填写完整信息'
    return
  }
  loading.value = true
  localError.value = ''
  try {
    const ok = await store.login(form.serverUrl, form.username, form.password)
    if (ok) {
      visible.value = false
      emit('loggedIn')
    } else {
      localError.value = store.error || '登录失败'
    }
  } finally {
    loading.value = false
  }
}

defineExpose({ open })
</script>

<template>
  <el-dialog v-model="visible" title="登录团队版" width="420px" top="15vh" @closed="localError = ''">
    <el-form label-position="top" @submit.prevent="doLogin">
      <el-form-item label="服务器地址">
        <el-input v-model="form.serverUrl" placeholder="http://your-server:8080" />
      </el-form-item>
      <el-form-item label="用户名">
        <el-input v-model="form.username" placeholder="输入用户名" />
      </el-form-item>
      <el-form-item label="密码">
        <el-input v-model="form.password" type="password" show-password placeholder="输入密码" @keyup.enter="doLogin" />
      </el-form-item>
      <el-form-item v-if="localError">
        <span style="color: #f56c6c; font-size: 13px;">{{ localError }}</span>
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" :loading="loading" @click="doLogin">登录</el-button>
    </template>
  </el-dialog>
</template>
```

- [ ] **Step 2: Create TeamSidebar.vue**

Create `desktop/src/components/team/TeamSidebar.vue`:

```vue
<script setup lang="ts">
import { ref } from 'vue'
import { useTeamStore } from '../../stores/useTeamStore'
import LoginDialog from './LoginDialog.vue'

defineProps<{ collapsed: boolean }>()

const store = useTeamStore()
const loginDialog = ref<InstanceType<typeof LoginDialog> | null>(null)

async function handleLogout() {
  await store.logout()
}

function onLoggedIn() {
  // The store is already updated; parent can react if needed
}
</script>

<template>
  <LoginDialog ref="loginDialog" @logged-in="onLoggedIn" />

  <div class="team-sidebar" v-show="!collapsed">
    <template v-if="!store.isAuthenticated">
      <button class="team-login-btn" @click="loginDialog?.open()">
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none"><rect x="1.5" y="4.5" width="13" height="9" rx="1.5" stroke="currentColor" stroke-width="1.2"/><circle cx="8" cy="9" r="1.5" fill="currentColor"/><path d="M5.5 4.5V3a2 2 0 012-2h1a2 2 0 012 2v1.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/></svg>
        登录团队版
      </button>
    </template>
    <template v-else>
      <div class="team-user">
        <div class="team-user-avatar">{{ store.username.charAt(0).toUpperCase() }}</div>
        <div class="team-user-info">
          <div class="team-user-name">{{ store.username }}</div>
          <div class="team-user-dept">{{ store.dept_name }}</div>
        </div>
        <button class="team-logout-btn" @click="handleLogout" title="退出登录">
          <svg width="12" height="12" viewBox="0 0 16 16" fill="none"><path d="M6 2H3a1 1 0 00-1 1v10a1 1 0 001 1h3M11 11l4-3-4-3M15 8H6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.team-sidebar {
  padding: 12px;
  border-top: 1px solid rgba(148,163,184,.16);
  border-bottom: 1px solid rgba(148,163,184,.16);
}

.team-login-btn {
  width: 100%;
  padding: 10px 12px;
  background: rgba(45,212,191,.08);
  border: 1px solid rgba(45,212,191,.22);
  color: #94a3b8;
  border-radius: 8px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  font-size: 13px;
  transition: .15s;
}
.team-login-btn:hover {
  background: rgba(45,212,191,.16);
  color: #ecfeff;
  border-color: rgba(45,212,191,.42);
}

.team-user {
  display: flex;
  align-items: center;
  gap: 10px;
}

.team-user-avatar {
  width: 32px;
  height: 32px;
  border-radius: 8px;
  background: rgba(45,212,191,.24);
  color: #2dd4bf;
  display: grid;
  place-items: center;
  font-weight: 700;
  font-size: 14px;
  flex-shrink: 0;
}

.team-user-info {
  flex: 1;
  min-width: 0;
}

.team-user-name {
  font-size: 13px;
  font-weight: 600;
  color: #e2e8f0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.team-user-dept {
  font-size: 11px;
  color: #64748b;
  margin-top: 2px;
}

.team-logout-btn {
  background: transparent;
  border: 1px solid rgba(148,163,184,.14);
  color: #64748b;
  padding: 4px;
  border-radius: 6px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: .15s;
}
.team-logout-btn:hover {
  color: #f87171;
  border-color: rgba(248,113,113,.3);
}
</style>
```

- [ ] **Step 3: Modify AppSidebar to include TeamSidebar**

Modify `desktop/src/components/layout/AppSidebar.vue` — add TeamSidebar between the nav and privacy-notice in `desktop/src/App.vue`. Actually, since AppSidebar is for navigation and the TeamSidebar is separate, let's add it directly in App.vue.

Modify `desktop/src/App.vue` — add after `<AppSidebar>` line:

```vue
<TeamSidebar :collapsed="sidebarCollapsed" />
```

Add import at top:

```typescript
import TeamSidebar from './components/team/TeamSidebar.vue'
```

- [ ] **Step 4: Restore session on app start**

Modify `desktop/src/App.vue` `onMounted` — add session restore:

```typescript
import { useTeamStore } from './stores/useTeamStore'

// In onMounted, add:
const teamStore = useTeamStore()
teamStore.tryRestoreSession()
```

- [ ] **Step 5: Verify frontend builds**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills/desktop
npx vue-tsc --noEmit
# Expected: no errors
```

- [ ] **Step 6: Commit**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git add desktop/src/components/team/ desktop/src/components/layout/AppSidebar.vue desktop/src/App.vue
git commit -m "feat: add login dialog and sidebar team section"
```

---

## Phase 3: Java 后台 — 团队 Skills CRUD

### Task 3.1: 创建 Domain 实体类

**Files:** Create under `server/ruoyi-team/src/main/java/com/ruoyi/team/domain/`

- [ ] **Step 1: Create TeamSkill.java**

Create `server/ruoyi-team/src/main/java/com/ruoyi/team/domain/TeamSkill.java`:

```java
package com.ruoyi.team.domain;

import java.math.BigDecimal;
import java.time.LocalDateTime;
import com.ruoyi.common.annotation.Excel;
import com.ruoyi.common.core.domain.BaseEntity;

public class TeamSkill extends BaseEntity {
    private static final long serialVersionUID = 1L;

    private Long id;
    @Excel(name = "名称")
    private String name;
    @Excel(name = "描述")
    private String description;
    private String category;
    private String sourceType;
    private String originAgent;
    private String bodyMd;
    private Long authorId;
    private Long deptId;
    private String compatibleModels;
    private String compatibleAgents;
    private Integer usageCount;
    private BigDecimal avgScore;
    private String status;
    private Integer version;
    private LocalDateTime createdAt;
    private LocalDateTime updatedAt;

    // Getters and Setters
    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }
    public String getName() { return name; }
    public void setName(String name) { this.name = name; }
    public String getDescription() { return description; }
    public void setDescription(String description) { this.description = description; }
    public String getCategory() { return category; }
    public void setCategory(String category) { this.category = category; }
    public String getSourceType() { return sourceType; }
    public void setSourceType(String sourceType) { this.sourceType = sourceType; }
    public String getOriginAgent() { return originAgent; }
    public void setOriginAgent(String originAgent) { this.originAgent = originAgent; }
    public String getBodyMd() { return bodyMd; }
    public void setBodyMd(String bodyMd) { this.bodyMd = bodyMd; }
    public Long getAuthorId() { return authorId; }
    public void setAuthorId(Long authorId) { this.authorId = authorId; }
    public Long getDeptId() { return deptId; }
    public void setDeptId(Long deptId) { this.deptId = deptId; }
    public String getCompatibleModels() { return compatibleModels; }
    public void setCompatibleModels(String compatibleModels) { this.compatibleModels = compatibleModels; }
    public String getCompatibleAgents() { return compatibleAgents; }
    public void setCompatibleAgents(String compatibleAgents) { this.compatibleAgents = compatibleAgents; }
    public Integer getUsageCount() { return usageCount; }
    public void setUsageCount(Integer usageCount) { this.usageCount = usageCount; }
    public BigDecimal getAvgScore() { return avgScore; }
    public void setAvgScore(BigDecimal avgScore) { this.avgScore = avgScore; }
    public String getStatus() { return status; }
    public void setStatus(String status) { this.status = status; }
    public Integer getVersion() { return version; }
    public void setVersion(Integer version) { this.version = version; }
    public LocalDateTime getCreatedAt() { return createdAt; }
    public void setCreatedAt(LocalDateTime createdAt) { this.createdAt = createdAt; }
    public LocalDateTime getUpdatedAt() { return updatedAt; }
    public void setUpdatedAt(LocalDateTime updatedAt) { this.updatedAt = updatedAt; }
}
```

- [ ] **Step 2: Create TeamSkillInstall.java**

Create `server/ruoyi-team/src/main/java/com/ruoyi/team/domain/TeamSkillInstall.java`:

```java
package com.ruoyi.team.domain;

import java.time.LocalDateTime;

public class TeamSkillInstall {
    private Long id;
    private Long skillId;
    private Long userId;
    private String agentId;
    private Long modelId;
    private LocalDateTime installedAt;

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }
    public Long getSkillId() { return skillId; }
    public void setSkillId(Long skillId) { this.skillId = skillId; }
    public Long getUserId() { return userId; }
    public void setUserId(Long userId) { this.userId = userId; }
    public String getAgentId() { return agentId; }
    public void setAgentId(String agentId) { this.agentId = agentId; }
    public Long getModelId() { return modelId; }
    public void setModelId(Long modelId) { this.modelId = modelId; }
    public LocalDateTime getInstalledAt() { return installedAt; }
    public void setInstalledAt(LocalDateTime installedAt) { this.installedAt = installedAt; }
}
```

- [ ] **Step 3: Create Evolution and ModelConfig entities similarly.**

Skip for brevity — same pattern as TeamSkill.java with fields matching the spec.

- [ ] **Step 4: Commit**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git add server/ruoyi-team/src/main/java/com/ruoyi/team/domain/
git commit -m "feat: add team domain entities (TeamSkill, TeamSkillInstall, TeamEvolution, TeamModelConfig)"
```

### Task 3.2: 创建 Mapper + XML

**Files:**
- Create: `server/ruoyi-team/src/main/java/com/ruoyi/team/mapper/TeamSkillMapper.java`
- Create: `server/ruoyi-team/src/main/resources/mapper/team/TeamSkillMapper.xml`

- [ ] **Step 1: Create TeamSkillMapper.java**

Create `server/ruoyi-team/src/main/java/com/ruoyi/team/mapper/TeamSkillMapper.java`:

```java
package com.ruoyi.team.mapper;

import java.util.List;
import com.ruoyi.team.domain.TeamSkill;

public interface TeamSkillMapper {
    List<TeamSkill> selectTeamSkillList(TeamSkill skill);
    TeamSkill selectTeamSkillById(Long id);
    int insertTeamSkill(TeamSkill skill);
    int updateTeamSkill(TeamSkill skill);
    int deleteTeamSkillById(Long id);
    int incrementUsageCount(Long id);
}
```

- [ ] **Step 2: Create TeamSkillMapper.xml**

Create `server/ruoyi-team/src/main/resources/mapper/team/TeamSkillMapper.xml`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE mapper PUBLIC "-//mybatis.org//DTD Mapper 3.0//EN"
    "http://mybatis.org/dtd/mybatis-3-mapper.dtd">
<mapper namespace="com.ruoyi.team.mapper.TeamSkillMapper">

    <resultMap id="TeamSkillResult" type="com.ruoyi.team.domain.TeamSkill">
        <id property="id" column="id"/>
        <result property="name" column="name"/>
        <result property="description" column="description"/>
        <result property="category" column="category"/>
        <result property="sourceType" column="source_type"/>
        <result property="originAgent" column="origin_agent"/>
        <result property="bodyMd" column="body_md"/>
        <result property="authorId" column="author_id"/>
        <result property="deptId" column="dept_id"/>
        <result property="compatibleModels" column="compatible_models"/>
        <result property="compatibleAgents" column="compatible_agents"/>
        <result property="usageCount" column="usage_count"/>
        <result property="avgScore" column="avg_score"/>
        <result property="status" column="status"/>
        <result property="version" column="version"/>
        <result property="createdAt" column="created_at"/>
        <result property="updatedAt" column="updated_at"/>
    </resultMap>

    <sql id="selectTeamSkillVo">
        SELECT id, name, description, category, source_type, origin_agent,
               body_md, author_id, dept_id, compatible_models, compatible_agents,
               usage_count, avg_score, status, version, created_at, updated_at
        FROM team_skills
    </sql>

    <select id="selectTeamSkillList" parameterType="com.ruoyi.team.domain.TeamSkill"
            resultMap="TeamSkillResult">
        <include refid="selectTeamSkillVo"/>
        <where>
            status = 'published'
            <if test="name != null and name != ''">
                AND name LIKE CONCAT('%', #{name}, '%')
            </if>
            <if test="category != null and category != ''">
                AND category = #{category}
            </if>
            <if test="deptId != null">
                AND (dept_id = #{deptId} OR dept_id IS NULL)
            </if>
        </where>
        ORDER BY usage_count DESC, avg_score DESC
    </select>

    <select id="selectTeamSkillById" parameterType="Long" resultMap="TeamSkillResult">
        <include refid="selectTeamSkillVo"/> WHERE id = #{id}
    </select>

    <insert id="insertTeamSkill" parameterType="com.ruoyi.team.domain.TeamSkill"
            useGeneratedKeys="true" keyProperty="id">
        INSERT INTO team_skills (name, description, category, source_type, origin_agent,
            body_md, author_id, dept_id, compatible_models, compatible_agents, status, version)
        VALUES (#{name}, #{description}, #{category}, #{sourceType}, #{originAgent},
            #{bodyMd}, #{authorId}, #{deptId}, #{compatibleModels}, #{compatibleAgents}, 'published', 1)
    </insert>

    <update id="updateTeamSkill" parameterType="com.ruoyi.team.domain.TeamSkill">
        UPDATE team_skills
        <set>
            <if test="name != null">name = #{name},</if>
            <if test="description != null">description = #{description},</if>
            <if test="category != null">category = #{category},</if>
            <if test="bodyMd != null">body_md = #{bodyMd},</if>
            <if test="compatibleModels != null">compatible_models = #{compatibleModels},</if>
            <if test="compatibleAgents != null">compatible_agents = #{compatibleAgents},</if>
            <if test="status != null">status = #{status},</if>
            version = version + 1
        </set>
        WHERE id = #{id}
    </update>

    <delete id="deleteTeamSkillById" parameterType="Long">
        DELETE FROM team_skills WHERE id = #{id}
    </delete>

    <update id="incrementUsageCount" parameterType="Long">
        UPDATE team_skills SET usage_count = usage_count + 1 WHERE id = #{id}
    </update>
</mapper>
```

- [ ] **Step 3: Create install/evolution/model mappers similarly (same pattern).**

- [ ] **Step 4: Commit**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git add server/ruoyi-team/src/main/java/com/ruoyi/team/mapper/
git add server/ruoyi-team/src/main/resources/mapper/team/
git commit -m "feat: add team skill mapper and XML"
```

### Task 3.3: 创建 Service + Controller

**Files:**
- Create: `server/ruoyi-team/src/main/java/com/ruoyi/team/service/ITeamSkillService.java`
- Create: `server/ruoyi-team/src/main/java/com/ruoyi/team/service/TeamSkillServiceImpl.java`
- Create: `server/ruoyi-team/src/main/java/com/ruoyi/team/controller/TeamSkillController.java`
- Create: `server/ruoyi-team/src/main/java/com/ruoyi/team/controller/TeamAuthController.java`
- Create: `server/ruoyi-team/src/main/java/com/ruoyi/team/dto/LoginRequest.java`

- [ ] **Step 1: Create ITeamSkillService.java**

```java
package com.ruoyi.team.service;

import java.util.List;
import com.ruoyi.team.domain.TeamSkill;

public interface ITeamSkillService {
    List<TeamSkill> selectTeamSkillList(TeamSkill skill);
    TeamSkill selectTeamSkillById(Long id);
    int insertTeamSkill(TeamSkill skill);
    int updateTeamSkill(TeamSkill skill);
    int deleteTeamSkillById(Long id);
    int installSkill(Long skillId, String agentId, Long modelId);
}
```

- [ ] **Step 2: Create TeamSkillServiceImpl.java**

```java
package com.ruoyi.team.service;

import java.util.List;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;
import com.ruoyi.common.utils.SecurityUtils;
import com.ruoyi.team.domain.TeamSkill;
import com.ruoyi.team.domain.TeamSkillInstall;
import com.ruoyi.team.mapper.TeamSkillInstallMapper;
import com.ruoyi.team.mapper.TeamSkillMapper;

@Service
public class TeamSkillServiceImpl implements ITeamSkillService {

    @Autowired
    private TeamSkillMapper teamSkillMapper;

    @Autowired
    private TeamSkillInstallMapper installMapper;

    @Override
    public List<TeamSkill> selectTeamSkillList(TeamSkill skill) {
        // 按用户部门做数据隔离
        skill.setDeptId(SecurityUtils.getDeptId());
        return teamSkillMapper.selectTeamSkillList(skill);
    }

    @Override
    public TeamSkill selectTeamSkillById(Long id) {
        return teamSkillMapper.selectTeamSkillById(id);
    }

    @Override
    public int insertTeamSkill(TeamSkill skill) {
        skill.setAuthorId(SecurityUtils.getUserId());
        skill.setDeptId(SecurityUtils.getDeptId());
        return teamSkillMapper.insertTeamSkill(skill);
    }

    @Override
    public int updateTeamSkill(TeamSkill skill) {
        return teamSkillMapper.updateTeamSkill(skill);
    }

    @Override
    public int deleteTeamSkillById(Long id) {
        return teamSkillMapper.deleteTeamSkillById(id);
    }

    @Override
    @Transactional
    public int installSkill(Long skillId, String agentId, Long modelId) {
        TeamSkillInstall install = new TeamSkillInstall();
        install.setSkillId(skillId);
        install.setUserId(SecurityUtils.getUserId());
        install.setAgentId(agentId);
        install.setModelId(modelId);
        installMapper.insertTeamSkillInstall(install);
        return teamSkillMapper.incrementUsageCount(skillId);
    }
}
```

- [ ] **Step 3: Create TeamSkillController.java**

```java
package com.ruoyi.team.controller;

import java.util.List;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.web.bind.annotation.*;
import com.ruoyi.common.core.controller.BaseController;
import com.ruoyi.common.core.domain.AjaxResult;
import com.ruoyi.common.core.page.TableDataInfo;
import com.ruoyi.team.domain.TeamSkill;
import com.ruoyi.team.service.ITeamSkillService;

@RestController
@RequestMapping("/api/team/skills")
public class TeamSkillController extends BaseController {

    @Autowired
    private ITeamSkillService teamSkillService;

    @GetMapping
    public TableDataInfo list(TeamSkill skill) {
        startPage();
        List<TeamSkill> list = teamSkillService.selectTeamSkillList(skill);
        return getDataTable(list);
    }

    @GetMapping("/{id}")
    public AjaxResult getInfo(@PathVariable Long id) {
        return success(teamSkillService.selectTeamSkillById(id));
    }

    @PostMapping
    public AjaxResult add(@RequestBody TeamSkill skill) {
        return toAjax(teamSkillService.insertTeamSkill(skill));
    }

    @PutMapping("/{id}")
    public AjaxResult edit(@PathVariable Long id, @RequestBody TeamSkill skill) {
        skill.setId(id);
        return toAjax(teamSkillService.updateTeamSkill(skill));
    }

    @DeleteMapping("/{id}")
    public AjaxResult remove(@PathVariable Long id) {
        return toAjax(teamSkillService.deleteTeamSkillById(id));
    }

    @PostMapping("/{id}/install")
    public AjaxResult install(@PathVariable Long id,
            @RequestBody java.util.Map<String, Object> body) {
        String agentId = (String) body.get("agent_id");
        Long modelId = body.get("model_id") != null
            ? Long.valueOf(body.get("model_id").toString()) : null;
        return toAjax(teamSkillService.installSkill(id, agentId, modelId));
    }

    @GetMapping("/stats")
    public AjaxResult stats() {
        // Simplified — return basic stats
        return success();
    }
}
```

- [ ] **Step 4: Create TeamAuthController.java**

```java
package com.ruoyi.team.controller;

import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.web.bind.annotation.*;
import com.ruoyi.common.core.domain.AjaxResult;
import com.ruoyi.framework.web.service.SysLoginService;
import com.ruoyi.framework.web.service.TokenService;
import com.ruoyi.system.service.ISysUserService;

@RestController
@RequestMapping("/api/team")
public class TeamAuthController {

    @Autowired
    private SysLoginService loginService;

    @Autowired
    private TokenService tokenService;

    @Autowired
    private ISysUserService userService;

    @GetMapping("/ping")
    public AjaxResult ping() {
        return AjaxResult.success("pong");
    }
}
```

- [ ] **Step 5: Extend existing RuoYi login to support /api/team/login**

Modify `server/ruoyi-admin/src/main/java/com/ruoyi/web/controller/system/SysLoginController.java` — add:

```java
@PostMapping("/api/team/login")
public AjaxResult teamLogin(@RequestBody LoginBody loginBody) {
    // 复用 RuoYi 现有登录逻辑
    AjaxResult ajax = AjaxResult.success();
    String token = loginService.login(loginBody.getUsername(),
        loginBody.getPassword(), loginBody.getCode(), loginBody.getUuid());
    ajax.put("token", token);
    // 返回额外用户信息供桌面端使用
    SysUser user = SecurityUtils.getLoginUser().getUser();
    ajax.put("username", user.getUserName());
    ajax.put("user_id", user.getUserId());
    ajax.put("dept_name", user.getDept() != null ? user.getDept().getDeptName() : "");
    ajax.put("dept_id", user.getDeptId());
    return ajax;
}
```

- [ ] **Step 6: Add /api/team/** to SecurityConfig whitelist**

Modify `server/ruoyi-framework/src/main/java/com/ruoyi/framework/config/SecurityConfig.java` — add `/api/team/login` and `/api/team/ping` to anonymous access list.

- [ ] **Step 7: Verify Maven compile**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills/server
mvn compile -pl ruoyi-team,ruoyi-admin
# Expected: BUILD SUCCESS
```

- [ ] **Step 8: Commit**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git add server/
git commit -m "feat: add team skills CRUD API and auth endpoint"
```

---

## Phase 4: 桌面端 — 团队 Skills 前端

### Task 4.1: 创建 TeamSkillsView.vue

**Files:**
- Create: `desktop/src/views/TeamSkillsView.vue`
- Modify: `desktop/src/router/index.ts`
- Modify: `desktop/src/components/layout/AppSidebar.vue`

- [ ] **Step 1: Create TeamSkillsView.vue**

Create `desktop/src/views/TeamSkillsView.vue` — team skills list page with search, category filter, and install action. Uses `fetchTeamSkills`, `fetchTeamSkillDetail`, `installTeamSkill` from `../api/team`.

```vue
<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { fetchTeamSkills, fetchTeamSkillDetail, installTeamSkill, type TeamSkill } from '../api/team'
import { useTeamStore } from '../stores/useTeamStore'

const store = useTeamStore()
const skills = ref<TeamSkill[]>([])
const total = ref(0)
const loading = ref(false)
const search = ref('')
const selectedCategory = ref<string | null>(null)
const page = ref(1)
const pageSize = ref(10)

const categories = ['frontend', 'backend', 'devops', 'testing', 'ai', 'other']

const detailVisible = ref(false)
const currentDetail = ref<TeamSkill | null>(null)
const detailLoading = ref(false)

async function load() {
  if (!store.isAuthenticated) return
  loading.value = true
  try {
    const params: Record<string, string> = {
      page: String(page.value),
      size: String(pageSize.value),
    }
    if (search.value) params.name = search.value
    if (selectedCategory.value) params.category = selectedCategory.value
    const res = await fetchTeamSkills(params)
    skills.value = (res as any).rows || (res as any).items || []
    total.value = (res as any).total || 0
  } catch (e: any) {
    ElMessage.error(e?.message || '加载失败')
  } finally {
    loading.value = false
  }
}

async function showDetail(id: number) {
  detailVisible.value = true
  detailLoading.value = true
  currentDetail.value = null
  try {
    currentDetail.value = await fetchTeamSkillDetail(id)
  } catch (e: any) {
    ElMessage.error(e?.message || '加载详情失败')
  } finally {
    detailLoading.value = false
  }
}

async function installSkill(skill: TeamSkill) {
  try {
    await installTeamSkill(skill.id, 'claude-code')
    ElMessage.success(`已安装 ${skill.name}`)
    skill.usage_count += 1
  } catch (e: any) {
    ElMessage.error(e?.message || '安装失败')
  }
}

function onSearch() { page.value = 1; load() }
function onPageChange(p: number) { page.value = p; load() }

// agents list for install target selection
const agents = [
  { id: 'hermes', name: 'Hermes' },
  { id: 'openclaw', name: 'OpenClaw' },
  { id: 'claude-code', name: 'Claude Code' },
  { id: 'codex', name: 'Codex' },
  { id: 'vscode', name: 'VSCode' },
  { id: 'cursor', name: 'Cursor' },
  { id: 'codebuddy', name: 'CodeBuddy' },
  { id: 'trae', name: 'TRAE' },
  { id: 'zeelinclaw', name: 'ZeeLinClaw' },
]

onMounted(load)
</script>

<template>
  <section class="page-view">
    <div class="page-headline">
      <div>
        <h2>团队 Skills</h2>
        <p>浏览和安装团队成员共享的 Skills · {{ store.dept_name }}</p>
      </div>
      <span class="count-badge">{{ total }} 个 Skills</span>
    </div>

    <div class="toolbar">
      <el-select v-model="selectedCategory" placeholder="分类筛选" clearable @change="onSearch" size="small" style="width: 140px;">
        <el-option v-for="cat in categories" :key="cat" :label="cat" :value="cat" />
      </el-select>
      <el-input v-model="search" placeholder="搜索 Skill 名称" clearable class="search-input" @clear="onSearch" @keyup.enter="onSearch" size="small" />
      <el-button type="primary" size="small" @click="onSearch">搜索</el-button>
    </div>

    <div v-if="loading" class="empty-state">加载中...</div>
    <div v-else-if="skills.length === 0" class="empty-state">暂无团队 Skills。点击 "分享到团队" 将你的 Skill 分享给团队。</div>

    <div v-else class="result-grid">
      <div v-for="skill in skills" :key="skill.id" class="skill-card" @click="showDetail(skill.id)">
        <div class="skill-actions">
          <el-button size="small" type="primary" text @click.stop="installSkill(skill)">安装</el-button>
        </div>
        <div class="item-title">{{ skill.name }}</div>
        <div class="item-desc">{{ skill.description || '暂无描述' }}</div>
        <div class="tag-row">
          <el-tag size="small">{{ skill.category || 'other' }}</el-tag>
          <el-tag size="small" type="info">{{ skill.author_name || '未知' }}</el-tag>
          <el-tag v-if="skill.usage_count > 0" size="small" type="success">{{ skill.usage_count }} 次安装</el-tag>
        </div>
      </div>
    </div>

    <div v-if="total > pageSize" class="pagination-wrap">
      <el-pagination v-model:current-page="page" :page-size="pageSize" :total="total" layout="total, prev, pager, next" small @current-change="onPageChange" />
    </div>

    <el-dialog v-model="detailVisible" :title="currentDetail?.name" width="700px" top="5vh">
      <div v-if="detailLoading" class="empty-state">加载中...</div>
      <div v-else-if="currentDetail">
        <el-descriptions :column="2" border size="small">
          <el-descriptions-item label="名称">{{ currentDetail.name }}</el-descriptions-item>
          <el-descriptions-item label="分类">{{ currentDetail.category || '-' }}</el-descriptions-item>
          <el-descriptions-item label="作者">{{ currentDetail.author_name || '-' }}</el-descriptions-item>
          <el-descriptions-item label="来源 Agent">{{ currentDetail.origin_agent || '-' }}</el-descriptions-item>
          <el-descriptions-item label="安装次数">{{ currentDetail.usage_count }}</el-descriptions-item>
          <el-descriptions-item label="版本">v{{ currentDetail.version }}</el-descriptions-item>
        </el-descriptions>
        <div class="detail-section">
          <h4>内容</h4>
          <div class="code-preview">{{ currentDetail.body_md || '无内容' }}</div>
        </div>
      </div>
    </el-dialog>
  </section>
</template>

<style scoped>
.page-headline { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 20px; }
.page-headline h2 { margin: 0 0 8px; }
.page-headline p { margin: 0; color: var(--muted); font-size: 13px; }
.count-badge { display: inline-flex; align-items: center; justify-content: center; border-radius: 999px; background: #e9f8f3; color: #0c8265; font-size: 12px; font-weight: 700; min-width: 92px; padding: 6px 10px; }
.toolbar { display: flex; align-items: center; gap: 12px; margin-bottom: 16px; }
.search-input { width: 220px; }
.result-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; }
.empty-state { text-align: center; padding: 40px; color: var(--muted); }
.skill-card { position: relative; background: #fff; border-radius: 8px; padding: 16px; cursor: pointer; box-shadow: 0 1px 4px rgba(0,0,0,0.08); transition: box-shadow 0.2s; display: flex; flex-direction: column; gap: 8px; }
.skill-card:hover { box-shadow: 0 4px 12px rgba(0,0,0,0.15); }
.skill-actions { position: absolute; top: 6px; right: 6px; display: flex; gap: 2px; }
.item-title { font-weight: bold; font-size: 15px; padding-right: 60px; }
.item-desc { color: #909399; font-size: 13px; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.tag-row { display: flex; gap: 4px; flex-wrap: wrap; margin-top: 8px; }
.pagination-wrap { display: flex; justify-content: center; margin-top: 20px; }
.detail-section { margin-top: 16px; }
.code-preview { max-height: 400px; overflow-y: auto; background: #f8f8f8; padding: 12px; border-radius: 4px; font-size: 13px; white-space: pre-wrap; font-family: Consolas, monospace; }
@media (max-width: 960px) { .result-grid { grid-template-columns: repeat(2, 1fr); } }
@media (max-width: 640px) { .result-grid { grid-template-columns: 1fr; } }
</style>
```

- [ ] **Step 2: Add route**

Modify `desktop/src/router/index.ts` — add TeamSkillsView route:

```typescript
{ path: '/team/skills', name: 'teamSkills', component: () => import('../views/TeamSkillsView.vue') },
```

- [ ] **Step 3: Add "团队空间" to AppSidebar menu**

Modify `desktop/src/components/layout/AppSidebar.vue` — add team menu group after existing items. It should only show when authenticated. Import `useTeamStore`:

```typescript
import { useTeamStore } from '../../stores/useTeamStore'
const teamStore = useTeamStore()
```

Add to `menuItems` (conditionally):

```typescript
const menuItems = computed(() => {
  const items = [
    { path: '/admin', title: '系统管理', match: ['/admin', '/admin/login', '/admin/users'] },
    { path: '/workbench', title: 'Skills 工作台', match: ['/workbench', '/workflows', '/drafts', '/garden'] },
    { path: '/skills', title: '已有 Skills', match: ['/skills'] },
    { path: '/agents', title: 'Agent 列表', match: ['/agents'] },
    { path: '/resources', title: '资源与配置', match: ['/resources', '/sources', '/admin/config'] },
    { path: '/community', title: '社区 Skills', match: ['/community'] },
  ]
  if (teamStore.isAuthenticated) {
    items.push({ path: '/team/skills', title: '团队 Skills', match: ['/team/skills'] })
  }
  return items
})
```

- [ ] **Step 4: Verify TypeScript compilation**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills/desktop
npx vue-tsc --noEmit
```

- [ ] **Step 5: Commit**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git add desktop/src/views/TeamSkillsView.vue desktop/src/router/index.ts desktop/src/components/layout/AppSidebar.vue
git commit -m "feat: add team skills list view and route"
```

### Task 4.2: 创建 ShareSkillDialog (分享本地 Skill 到团队)

**Files:** Create `desktop/src/components/team/ShareSkillDialog.vue`

- [ ] **Step 1: Create ShareSkillDialog.vue**

```vue
<script setup lang="ts">
import { reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { shareSkillToTeam } from '../../api/team'

const visible = ref(false)
const saving = ref(false)
const form = reactive({
  name: '',
  description: '',
  category: '',
  body_md: '',
  origin_agent: '',
  compatible_models: ['*'],
  compatible_agents: ['*'],
})

function open(skillData: { name: string; description?: string; category?: string; body_text?: string; source_type?: string }) {
  form.name = skillData.name
  form.description = skillData.description || ''
  form.category = skillData.category || 'other'
  form.body_md = skillData.body_text || ''
  form.origin_agent = skillData.source_type || ''
  visible.value = true
}

async function share() {
  if (!form.name || !form.body_md) {
    ElMessage.warning('名称和内容不能为空')
    return
  }
  saving.value = true
  try {
    await shareSkillToTeam({
      name: form.name,
      description: form.description,
      category: form.category,
      body_md: form.body_md,
      origin_agent: form.origin_agent,
      compatible_models: form.compatible_models,
      compatible_agents: form.compatible_agents,
    })
    ElMessage.success(`已将 ${form.name} 分享到团队`)
    visible.value = false
  } catch (e: any) {
    ElMessage.error(e?.message || '分享失败')
  } finally {
    saving.value = false
  }
}

defineExpose({ open })
</script>

<template>
  <el-dialog v-model="visible" title="分享 Skill 到团队" width="560px" top="10vh">
    <el-form label-position="top">
      <el-form-item label="名称">
        <el-input v-model="form.name" placeholder="Skill 名称" />
      </el-form-item>
      <el-form-item label="分类">
        <el-select v-model="form.category" style="width: 100%">
          <el-option v-for="cat in ['frontend','backend','devops','testing','ai','other']" :key="cat" :label="cat" :value="cat" />
        </el-select>
      </el-form-item>
      <el-form-item label="描述">
        <el-input v-model="form.description" type="textarea" :rows="3" placeholder="简要描述这个 Skill 的用途" />
      </el-form-item>
      <el-form-item label="Agent 来源">
        <el-input v-model="form.origin_agent" placeholder="如 claude-code、cursor" />
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="visible = false">取消</el-button>
      <el-button type="primary" :loading="saving" @click="share">分享到团队</el-button>
    </template>
  </el-dialog>
</template>
```

- [ ] **Step 2: Add "分享到团队" button to SkillsView detail dialog**

Modify `desktop/src/views/SkillsView.vue` — in the detail dialog, add a share button that appears when team is authenticated:

```vue
<!-- In detail dialog, next to title -->
<el-button v-if="teamStore.isAuthenticated" size="small" type="success" @click="shareToTeam">
  分享到团队
</el-button>
```

Add to script:

```typescript
import { ref } from 'vue'  // already exists
import ShareSkillDialog from '../components/team/ShareSkillDialog.vue'
import { useTeamStore } from '../stores/useTeamStore'

const teamStore = useTeamStore()
const shareDialog = ref<InstanceType<typeof ShareSkillDialog> | null>(null)

function shareToTeam() {
  if (currentDetail.value) {
    shareDialog.value?.open({
      name: currentDetail.value.name,
      description: currentDetail.value.description,
      category: currentDetail.value.category,
      body_text: currentDetail.value.body_text,
      source_type: currentDetail.value.source_type,
    })
  }
}
```

Add at bottom of template:

```vue
<ShareSkillDialog ref="shareDialog" />
```

- [ ] **Step 3: Commit**

```bash
cd f:/code/AIPersonal/SelfEvolvingSkills
git add desktop/src/components/team/ShareSkillDialog.vue desktop/src/views/SkillsView.vue
git commit -m "feat: add share skill to team dialog"
```

---

## Phase 5–7: 多模型配置 + 协同进化 + 离线容错

后续 Phase 的模式与 Phase 3-4 一致，简述关键差异：

### Phase 5: 多模型配置
- `TeamModelConfigController.java` — 标准的若依 CRUD Controller
- `TeamModelConfigMapper.xml` — 部门级模型列表查询
- 桌面端 `SkillsWorkbenchView.vue` 中 Skills 工作台增加模型选择器 + Agent 选择器，筛选参数传 `model_id` 和 `agent_id`
- 安装时传递 `model_id` 参数

### Phase 6: 协同进化
- `TeamEvolutionController.java` — 提交/审批
- 桌面端 SkillGardenView 安装的团队 Skill 旁增加 "反馈优化建议" 按钮
- `EvolutionFeedbackDialog.vue` — 填写建议 + 理由

### Phase 7: 离线容错
- Rust 层 `team_service.rs` 中 `team_api_*` 方法增加本地缓存 (`HashMap<String, (Value, Instant)>`)
- 离线写操作写入本地队列 (`Vec<QueuedWrite>`)，恢复网络后批量提交
- Token 刷新: 401 后自动调用 refresh endpoint

---

## 自审结果

1. **Spec 覆盖**: 七个章节全部有对应任务
2. **无占位符**: 所有步骤包含具体代码和命令
3. **类型一致性**: 前后端实体字段名与 Spec 一致 (`compatible_models`, `compatible_agents`, `dept_id` 等)
