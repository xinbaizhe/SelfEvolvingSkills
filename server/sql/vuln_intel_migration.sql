-- MySQL 8 migration for vulnerability scanner model selection and intelligence list.
-- Apply after backing up the database.

ALTER TABLE vuln_scan_jobs ADD COLUMN model_type VARCHAR(20) COMMENT 'department/personal' AFTER low_count;
ALTER TABLE vuln_scan_jobs ADD COLUMN model_id BIGINT COMMENT 'department model id' AFTER model_type;
ALTER TABLE vuln_scan_jobs ADD COLUMN progress_step VARCHAR(50) COMMENT 'current scan step' AFTER model_id;
ALTER TABLE vuln_scan_jobs ADD COLUMN progress_text LONGTEXT COMMENT 'scan execution records' AFTER progress_step;
ALTER TABLE vuln_scan_jobs MODIFY COLUMN progress_text LONGTEXT COMMENT 'scan execution records';

CREATE TABLE IF NOT EXISTS vuln_intel (
    id              BIGINT AUTO_INCREMENT PRIMARY KEY,
    source          VARCHAR(50) NOT NULL,
    cve_id          VARCHAR(50) NOT NULL,
    title           VARCHAR(500),
    vuln_type       VARCHAR(100),
    severity        VARCHAR(20),
    vendor_project  VARCHAR(200),
    product         VARCHAR(200),
    description     TEXT,
    reference_url   VARCHAR(1000),
    published_at    DATETIME,
    updated_at      DATETIME,
    created_at      DATETIME DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uk_vuln_intel_source_cve (source, cve_id),
    INDEX idx_vuln_intel_type (vuln_type),
    INDEX idx_vuln_intel_severity (severity),
    INDEX idx_vuln_intel_published (published_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COMMENT='漏洞情报库';
