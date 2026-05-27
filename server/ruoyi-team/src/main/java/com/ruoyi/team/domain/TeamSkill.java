package com.ruoyi.team.domain;

import java.math.BigDecimal;
import java.time.LocalDateTime;
import com.ruoyi.common.core.domain.BaseEntity;

public class TeamSkill extends BaseEntity {
    private static final long serialVersionUID = 1L;

    private Long id;
    private String name;
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
