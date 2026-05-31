package com.ruoyi.team.domain;

import java.time.LocalDateTime;

public class DepMonitor {
    private Long id;
    private Long userId;
    private Long deptId;
    private String name;
    private String ecosystem;
    private String packageName;
    private String version;
    private Integer vulnCount;
    private Integer poisoningCount;
    private LocalDateTime lastCheckedAt;
    private LocalDateTime createdAt;
    private LocalDateTime updatedAt;

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }
    public Long getUserId() { return userId; }
    public void setUserId(Long userId) { this.userId = userId; }
    public Long getDeptId() { return deptId; }
    public void setDeptId(Long deptId) { this.deptId = deptId; }
    public String getName() { return name; }
    public void setName(String name) { this.name = name; }
    public String getEcosystem() { return ecosystem; }
    public void setEcosystem(String ecosystem) { this.ecosystem = ecosystem; }
    public String getPackageName() { return packageName; }
    public void setPackageName(String packageName) { this.packageName = packageName; }
    public String getVersion() { return version; }
    public void setVersion(String version) { this.version = version; }
    public Integer getVulnCount() { return vulnCount; }
    public void setVulnCount(Integer vulnCount) { this.vulnCount = vulnCount; }
    public Integer getPoisoningCount() { return poisoningCount; }
    public void setPoisoningCount(Integer poisoningCount) { this.poisoningCount = poisoningCount; }
    public LocalDateTime getLastCheckedAt() { return lastCheckedAt; }
    public void setLastCheckedAt(LocalDateTime lastCheckedAt) { this.lastCheckedAt = lastCheckedAt; }
    public LocalDateTime getCreatedAt() { return createdAt; }
    public void setCreatedAt(LocalDateTime createdAt) { this.createdAt = createdAt; }
    public LocalDateTime getUpdatedAt() { return updatedAt; }
    public void setUpdatedAt(LocalDateTime updatedAt) { this.updatedAt = updatedAt; }
}
