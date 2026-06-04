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
    zip_file_name   VARCHAR(255) COMMENT '服务器存储的原始zip文件名',
    zip_file_path   VARCHAR(1000) COMMENT '服务器zip相对路径',
    zip_file_size   BIGINT COMMENT 'zip文件大小',
    author_id       BIGINT NOT NULL COMMENT '上传者',
    created_by      VARCHAR(64) COMMENT '创建人',
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
    INDEX idx_team_skills_author (author_id),
    INDEX idx_team_skills_created (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='团队Skills';

-- Existing MySQL 8 deployments can apply this migration after confirming the column/index do not exist:
-- ALTER TABLE team_skills ADD COLUMN created_by VARCHAR(64) COMMENT '创建人' AFTER author_id;
-- ALTER TABLE team_skills ADD COLUMN zip_file_name VARCHAR(255) COMMENT '服务器存储的原始zip文件名' AFTER body_md;
-- ALTER TABLE team_skills ADD COLUMN zip_file_path VARCHAR(1000) COMMENT '服务器zip相对路径' AFTER zip_file_name;
-- ALTER TABLE team_skills ADD COLUMN zip_file_size BIGINT COMMENT 'zip文件大小' AFTER zip_file_path;
-- CREATE INDEX idx_team_skills_created ON team_skills (created_at);

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

-- 漏洞扫描任务
DROP TABLE IF EXISTS vuln_scan_jobs;
CREATE TABLE vuln_scan_jobs (
    id              BIGINT AUTO_INCREMENT PRIMARY KEY,
    scan_type       VARCHAR(20)  NOT NULL COMMENT '扫描类型: url/code',
    target          VARCHAR(1000) NOT NULL COMMENT '扫描目标URL或目录',
    status          VARCHAR(20)  DEFAULT 'running' COMMENT 'running/completed',
    total_findings  INT DEFAULT 0,
    critical_count  INT DEFAULT 0,
    high_count      INT DEFAULT 0,
    medium_count    INT DEFAULT 0,
    low_count       INT DEFAULT 0,
    model_type      VARCHAR(20) COMMENT 'department/personal',
    model_id        BIGINT COMMENT 'department model id',
    progress_step   VARCHAR(50) COMMENT 'current scan step',
    progress_text   LONGTEXT COMMENT 'scan execution records',
    user_id         BIGINT NOT NULL,
    dept_id         BIGINT,
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_vuln_scan_user (user_id),
    INDEX idx_vuln_scan_type (scan_type),
    INDEX idx_vuln_scan_created (created_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='漏洞扫描任务';

-- 漏洞发现明细
DROP TABLE IF EXISTS vuln_scan_findings;
CREATE TABLE vuln_scan_findings (
    id          BIGINT AUTO_INCREMENT PRIMARY KEY,
    job_id      BIGINT NOT NULL,
    severity    VARCHAR(20) NOT NULL COMMENT 'CRITICAL/HIGH/MEDIUM/LOW',
    type        VARCHAR(100) NOT NULL,
    location    VARCHAR(1000),
    description TEXT,
    suggestion  TEXT,
    confidence  INT DEFAULT NULL COMMENT '置信度0-100，NULL表示未评估',
    INDEX idx_vuln_findings_job (job_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='漏洞发现明细';

-- ALTER TABLE for confidence column migration (existing databases)
-- ALTER TABLE vuln_scan_findings ADD COLUMN confidence INT DEFAULT NULL COMMENT '置信度0-100' AFTER suggestion;

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
    recipient_id BIGINT COMMENT '个人转赠接收用户',
    is_active   TINYINT(1) DEFAULT 1,
    created_at  DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_team_model_recipient (recipient_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='团队模型配置';
