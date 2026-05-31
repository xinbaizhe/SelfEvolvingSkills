package com.ruoyi.team.domain;

import java.time.LocalDateTime;

public class VulnIntel {
    private Long id;
    private String source;
    private String cveId;
    private String title;
    private String vulnType;
    private String severity;
    private String vendorProject;
    private String product;
    private String ecosystem;
    private Boolean isPoisoning;
    private String aliases;
    private String description;
    private String referenceUrl;
    private LocalDateTime publishedAt;
    private LocalDateTime updatedAt;
    private LocalDateTime createdAt;

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }
    public String getSource() { return source; }
    public void setSource(String source) { this.source = source; }
    public String getCveId() { return cveId; }
    public void setCveId(String cveId) { this.cveId = cveId; }
    public String getTitle() { return title; }
    public void setTitle(String title) { this.title = title; }
    public String getVulnType() { return vulnType; }
    public void setVulnType(String vulnType) { this.vulnType = vulnType; }
    public String getSeverity() { return severity; }
    public void setSeverity(String severity) { this.severity = severity; }
    public String getVendorProject() { return vendorProject; }
    public void setVendorProject(String vendorProject) { this.vendorProject = vendorProject; }
    public String getProduct() { return product; }
    public void setProduct(String product) { this.product = product; }
    public String getEcosystem() { return ecosystem; }
    public void setEcosystem(String ecosystem) { this.ecosystem = ecosystem; }
    public Boolean getIsPoisoning() { return isPoisoning; }
    public void setIsPoisoning(Boolean isPoisoning) { this.isPoisoning = isPoisoning; }
    public String getAliases() { return aliases; }
    public void setAliases(String aliases) { this.aliases = aliases; }
    public String getDescription() { return description; }
    public void setDescription(String description) { this.description = description; }
    public String getReferenceUrl() { return referenceUrl; }
    public void setReferenceUrl(String referenceUrl) { this.referenceUrl = referenceUrl; }
    public LocalDateTime getPublishedAt() { return publishedAt; }
    public void setPublishedAt(LocalDateTime publishedAt) { this.publishedAt = publishedAt; }
    public LocalDateTime getUpdatedAt() { return updatedAt; }
    public void setUpdatedAt(LocalDateTime updatedAt) { this.updatedAt = updatedAt; }
    public LocalDateTime getCreatedAt() { return createdAt; }
    public void setCreatedAt(LocalDateTime createdAt) { this.createdAt = createdAt; }
}
