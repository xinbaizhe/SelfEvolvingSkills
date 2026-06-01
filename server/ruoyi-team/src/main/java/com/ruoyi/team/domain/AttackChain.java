package com.ruoyi.team.domain;

import java.util.ArrayList;
import java.util.List;

public class AttackChain {
    private String chainId;
    private List<ChainStep> steps = new ArrayList<>();
    private String status = "active";
    private String impact;

    public record ChainStep(String action, String result, String timestamp) {}

    public AttackChain(String chainId) { this.chainId = chainId; }

    public String getChainId() { return chainId; }
    public List<ChainStep> getSteps() { return steps; }
    public String getStatus() { return status; }
    public void setStatus(String status) { this.status = status; }
    public String getImpact() { return impact; }
    public void setImpact(String impact) { this.impact = impact; }

    public void addStep(String action, String result) {
        steps.add(new ChainStep(action, result, java.time.LocalDateTime.now().toString()));
    }
}
