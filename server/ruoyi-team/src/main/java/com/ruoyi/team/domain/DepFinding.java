package com.ruoyi.team.domain;

import java.time.LocalDateTime;

public class DepFinding {
    private Long id;
    private Long depId;
    private Long intelId;
    private String cveId;
    private String title;
    private String severity;
    private Boolean isPoisoning;
    private String referenceUrl;
    private LocalDateTime createdAt;

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }
    public Long getDepId() { return depId; }
    public void setDepId(Long depId) { this.depId = depId; }
    public Long getIntelId() { return intelId; }
    public void setIntelId(Long intelId) { this.intelId = intelId; }
    public String getCveId() { return cveId; }
    public void setCveId(String cveId) { this.cveId = cveId; }
    public String getTitle() { return title; }
    public void setTitle(String title) { this.title = title; }
    public String getSeverity() { return severity; }
    public void setSeverity(String severity) { this.severity = severity; }
    public Boolean getIsPoisoning() { return isPoisoning; }
    public void setIsPoisoning(Boolean isPoisoning) { this.isPoisoning = isPoisoning; }
    public String getReferenceUrl() { return referenceUrl; }
    public void setReferenceUrl(String referenceUrl) { this.referenceUrl = referenceUrl; }
    public LocalDateTime getCreatedAt() { return createdAt; }
    public void setCreatedAt(LocalDateTime createdAt) { this.createdAt = createdAt; }
}
