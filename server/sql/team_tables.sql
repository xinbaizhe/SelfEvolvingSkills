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
