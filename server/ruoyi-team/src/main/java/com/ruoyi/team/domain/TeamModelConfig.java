package com.ruoyi.team.domain;

import java.time.LocalDateTime;
import com.ruoyi.common.core.domain.BaseEntity;

public class TeamModelConfig extends BaseEntity {
    private static final long serialVersionUID = 1L;

    private Long id;
    private String name;
    private String provider;
    private String baseUrl;
    private String model;
    private String apiKeyHash;
    private Long deptId;
    private String deptName;
    private Long createdBy;
    private String createdByName;
    private Long recipientId;
    private String recipientName;
    private Integer isActive;
    private LocalDateTime createdAt;
    private Boolean includeChildren;

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }
    public String getName() { return name; }
    public void setName(String name) { this.name = name; }
    public String getProvider() { return provider; }
    public void setProvider(String provider) { this.provider = provider; }
    public String getBaseUrl() { return baseUrl; }
    public void setBaseUrl(String baseUrl) { this.baseUrl = baseUrl; }
    public String getModel() { return model; }
    public void setModel(String model) { this.model = model; }
    public String getApiKeyHash() { return apiKeyHash; }
    public void setApiKeyHash(String apiKeyHash) { this.apiKeyHash = apiKeyHash; }
    public Long getDeptId() { return deptId; }
    public void setDeptId(Long deptId) { this.deptId = deptId; }
    public String getDeptName() { return deptName; }
    public void setDeptName(String deptName) { this.deptName = deptName; }
    public Long getCreatedBy() { return createdBy; }
    public void setCreatedBy(Long createdBy) { this.createdBy = createdBy; }
    public String getCreatedByName() { return createdByName; }
    public void setCreatedByName(String createdByName) { this.createdByName = createdByName; }
    public Long getRecipientId() { return recipientId; }
    public void setRecipientId(Long recipientId) { this.recipientId = recipientId; }
    public String getRecipientName() { return recipientName; }
    public void setRecipientName(String recipientName) { this.recipientName = recipientName; }
    public String getSourceType() { return deptId == null ? "personal" : "department"; }
    public Integer getIsActive() { return isActive; }
    public void setIsActive(Integer isActive) { this.isActive = isActive; }
    public LocalDateTime getCreatedAt() { return createdAt; }
    public void setCreatedAt(LocalDateTime createdAt) { this.createdAt = createdAt; }
    public Boolean getIncludeChildren() { return includeChildren; }
    public void setIncludeChildren(Boolean includeChildren) { this.includeChildren = includeChildren; }
}
