package com.ruoyi.team.domain;

import java.time.LocalDateTime;
import com.ruoyi.common.core.domain.BaseEntity;

public class TeamSkillInstall extends BaseEntity {
    private static final long serialVersionUID = 1L;

    private Long id;
    private Long skillId;
    private Long userId;
    private String agentId;
    private Long modelId;
    private LocalDateTime installedAt;

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }
    public Long getSkillId() { return skillId; }
    public void setSkillId(Long skillId) { this.skillId = skillId; }
    public Long getUserId() { return userId; }
    public void setUserId(Long userId) { this.userId = userId; }
    public String getAgentId() { return agentId; }
    public void setAgentId(String agentId) { this.agentId = agentId; }
    public Long getModelId() { return modelId; }
    public void setModelId(Long modelId) { this.modelId = modelId; }
    public LocalDateTime getInstalledAt() { return installedAt; }
    public void setInstalledAt(LocalDateTime installedAt) { this.installedAt = installedAt; }
}
