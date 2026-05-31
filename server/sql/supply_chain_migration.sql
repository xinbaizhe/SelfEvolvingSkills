-- Supply chain intelligence & dependency monitoring migration
-- Apply after backing up the database.

ALTER TABLE vuln_intel ADD COLUMN ecosystem VARCHAR(50) COMMENT 'ecosystem (npm/maven/pypi/...)' AFTER product;
ALTER TABLE vuln_intel ADD COLUMN is_poisoning TINYINT(1) DEFAULT 0 COMMENT '1=supply chain poisoning' AFTER ecosystem;
ALTER TABLE vuln_intel ADD COLUMN aliases JSON COMMENT 'alternative IDs like ["GHSA-xxxx","MAL-2023-1234"]' AFTER is_poisoning;

CREATE TABLE IF NOT EXISTS dep_monitor (
    id              BIGINT AUTO_INCREMENT PRIMARY KEY,
    user_id         BIGINT NOT NULL,
    dept_id         BIGINT,
    name            VARCHAR(200) COMMENT 'snapshot name, e.g. project name',
    ecosystem       VARCHAR(50) NOT NULL,
    package_name    VARCHAR(300) NOT NULL,
    version         VARCHAR(100),
    vuln_count      INT DEFAULT 0,
    poisoning_count INT DEFAULT 0,
    last_checked_at DATETIME,
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at      DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_dep_monitor_user (user_id),
    INDEX idx_dep_monitor_pkg (ecosystem, package_name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='依赖监控快照';

CREATE TABLE IF NOT EXISTS dep_finding (
    id              BIGINT AUTO_INCREMENT PRIMARY KEY,
    dep_id          BIGINT NOT NULL,
    intel_id        BIGINT COMMENT 'optional FK to vuln_intel',
    cve_id          VARCHAR(50),
    title           VARCHAR(500),
    severity        VARCHAR(20),
    is_poisoning    TINYINT(1) DEFAULT 0,
    reference_url   VARCHAR(1000),
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_dep_finding_dep (dep_id),
    FOREIGN KEY (dep_id) REFERENCES dep_monitor(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='依赖关联漏洞/投毒';
