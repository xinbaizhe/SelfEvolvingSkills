package com.ruoyi.team.service.impl;

import java.util.*;
import java.util.stream.Collectors;

import com.ruoyi.team.domain.*;

public class AgentMemory {

    private final Deque<String> shortTermMemory = new ArrayDeque<>();
    private static final int MAX_SHORT_TERM = 20;

    private final Map<String, DiscoveredEndpoint> endpoints = new LinkedHashMap<>();
    private final Map<String, AgentParam> params = new LinkedHashMap<>();
    private final List<VulnFinding> vulns = new ArrayList<>();
    private final Map<String, CredentialState> credentials = new LinkedHashMap<>();
    private final List<AttackChain> chains = new ArrayList<>();

    public void remember(String observation) {
        shortTermMemory.addLast(observation);
        while (shortTermMemory.size() > MAX_SHORT_TERM) {
            shortTermMemory.removeFirst();
        }
    }

    public List<String> recentObservations(int n) {
        List<String> all = new ArrayList<>(shortTermMemory);
        int from = Math.max(0, all.size() - n);
        return all.subList(from, all.size());
    }

    public void addEndpoint(DiscoveredEndpoint ep) {
        String key = ep.method() + "|" + ep.url();
        endpoints.merge(key, ep, (old, cur) -> cur);
    }

    public List<DiscoveredEndpoint> getEndpoints() {
        return new ArrayList<>(endpoints.values());
    }

    public List<DiscoveredEndpoint> findEndpoints(String keyword) {
        if (keyword == null || keyword.isEmpty()) return getEndpoints();
        String lower = keyword.toLowerCase();
        return endpoints.values().stream()
            .filter(e -> e.url().toLowerCase().contains(lower))
            .collect(Collectors.toList());
    }

    public void addParam(AgentParam param) {
        String key = param.endpoint() + "|" + param.paramName();
        params.merge(key, param, (old, cur) -> cur);
    }

    public List<AgentParam> getUntestedParams() {
        return params.values().stream()
            .filter(p -> !p.isTested())
            .collect(Collectors.toList());
    }

    public List<AgentParam> findParams(String endpointUrl) {
        return params.values().stream()
            .filter(p -> p.endpoint().contains(endpointUrl))
            .collect(Collectors.toList());
    }

    public record AgentParam(String endpoint, String paramName, String paramType,
                             boolean isInjectable, boolean isTested) {
        public AgentParam markTested() {
            return new AgentParam(endpoint, paramName, paramType, isInjectable, true);
        }
    }

    public void addVuln(VulnFinding v) { vulns.add(v); }

    public List<VulnFinding> getVulns() { return new ArrayList<>(vulns); }

    public List<VulnFinding> getConfirmedVulns() {
        return vulns.stream()
            .filter(v -> "CRITICAL".equalsIgnoreCase(v.getSeverity())
                      || "HIGH".equalsIgnoreCase(v.getSeverity()))
            .collect(Collectors.toList());
    }

    public void addCredential(CredentialState cred) {
        credentials.put(cred.credId(), cred);
    }

    public CredentialState getCredential(String credId) {
        return credentials.get(credId);
    }

    public List<CredentialState> getActiveCredentials() {
        return credentials.values().stream()
            .filter(CredentialState::sessionValid)
            .collect(Collectors.toList());
    }

    public List<CredentialState> getAllCredentials() {
        return new ArrayList<>(credentials.values());
    }

    public AttackChain createChain(String chainId) {
        AttackChain chain = new AttackChain(chainId);
        chains.add(chain);
        return chain;
    }

    public List<AttackChain> getActiveChains() {
        return chains.stream()
            .filter(c -> "active".equals(c.getStatus()))
            .collect(Collectors.toList());
    }

    public List<AttackChain> getAllChains() { return new ArrayList<>(chains); }

    public String summarize() {
        StringBuilder sb = new StringBuilder();
        sb.append("已发现端点: ").append(endpoints.size()).append("个\n");
        for (DiscoveredEndpoint ep : endpoints.values()) {
            sb.append("  ").append(ep.summary()).append("\n");
        }
        sb.append("已发现漏洞: ").append(vulns.size()).append("个\n");
        for (VulnFinding v : vulns) {
            sb.append("  [").append(v.getSeverity()).append("] ").append(v.getType())
              .append(" @ ").append(v.getLocation()).append("\n");
        }
        sb.append("活跃攻击链: ").append(getActiveChains().size()).append("条\n");
        sb.append("凭据: ").append(credentials.size()).append("组\n");
        return sb.toString();
    }

    public void injectRuleFindings(List<VulnFinding> ruleFindings,
                                    List<String> visitedUrls) {
        if (visitedUrls != null) {
            for (String url : visitedUrls) {
                addEndpoint(new DiscoveredEndpoint(url, "GET", "text/html", false,
                    "rule_scan", List.of(), 200, "", java.time.LocalDateTime.now()));
            }
        }
        if (ruleFindings != null) {
            for (VulnFinding f : ruleFindings) {
                addVuln(f);
                if (f.getLocation() != null && !f.getLocation().isBlank()) {
                    addEndpoint(new DiscoveredEndpoint(f.getLocation(), "GET", "unknown",
                        false, "rule_scan:" + f.getType(), List.of(), 0, f.getDescription(),
                        java.time.LocalDateTime.now()));
                }
            }
        }
    }
}
