package com.ruoyi.team.domain;

import java.time.LocalDateTime;
import java.util.List;

public class VulnScanJob {
    private Long id;
    private String scanType;
    private String target;
    private String status;
    private Integer totalFindings;
    private Integer criticalCount;
    private Integer highCount;
    private Integer mediumCount;
    private Integer lowCount;
    private List<VulnFinding> findings;
    private Long userId;
    private Long deptId;
    private LocalDateTime createdAt;

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }
    public String getScanType() { return scanType; }
    public void setScanType(String scanType) { this.scanType = scanType; }
    public String getTarget() { return target; }
    public void setTarget(String target) { this.target = target; }
    public String getStatus() { return status; }
    public void setStatus(String status) { this.status = status; }
    public Integer getTotalFindings() { return totalFindings; }
    public void setTotalFindings(Integer totalFindings) { this.totalFindings = totalFindings; }
    public Integer getCriticalCount() { return criticalCount; }
    public void setCriticalCount(Integer criticalCount) { this.criticalCount = criticalCount; }
    public Integer getHighCount() { return highCount; }
    public void setHighCount(Integer highCount) { this.highCount = highCount; }
    public Integer getMediumCount() { return mediumCount; }
    public void setMediumCount(Integer mediumCount) { this.mediumCount = mediumCount; }
    public Integer getLowCount() { return lowCount; }
    public void setLowCount(Integer lowCount) { this.lowCount = lowCount; }
    public List<VulnFinding> getFindings() { return findings; }
    public void setFindings(List<VulnFinding> findings) { this.findings = findings; }
    public Long getUserId() { return userId; }
    public void setUserId(Long userId) { this.userId = userId; }
    public Long getDeptId() { return deptId; }
    public void setDeptId(Long deptId) { this.deptId = deptId; }
    public LocalDateTime getCreatedAt() { return createdAt; }
    public void setCreatedAt(LocalDateTime createdAt) { this.createdAt = createdAt; }
}
