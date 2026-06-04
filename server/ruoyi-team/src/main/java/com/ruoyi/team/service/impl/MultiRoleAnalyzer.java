package com.ruoyi.team.service.impl;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.*;
import java.util.concurrent.*;
import java.util.regex.Pattern;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import com.ruoyi.team.domain.VulnFinding;

/**
 * Parallel multi-role LLM vulnerability analyzer.
 * Dispatches 6 specialized security roles concurrently via CompletableFuture,
 * each using a distinct system prompt to analyze the target from a different angle.
 * Results are aggregated by (type, location) deduplication, keeping the highest severity.
 */
public class MultiRoleAnalyzer {

    private static final Logger log = LoggerFactory.getLogger(MultiRoleAnalyzer.class);
    private static final ObjectMapper mapper = new ObjectMapper();
    private static final Duration REQUEST_TIMEOUT = Duration.ofSeconds(30);
    private static final Pattern JSON_OBJECT = Pattern.compile("\\{[^{}]*\"severity\"[^{}]*\\}", Pattern.DOTALL);

    private final String baseUrl;
    private final String model;
    private final String apiKey;
    private final HttpClient httpClient;
    private final ExecutorService executor;

    public MultiRoleAnalyzer(String baseUrl, String model, String apiKey, HttpClient httpClient) {
        this.baseUrl = baseUrl;
        this.model = model;
        this.apiKey = apiKey;
        this.httpClient = httpClient;
        this.executor = Executors.newFixedThreadPool(6);
    }

    // ── Discovery mode: each role independently analyzes raw HTTP response ──

    public List<VulnFinding> multiRoleDiscover(String url, String responseBody,
                                                Map<String, List<String>> headers) {
        List<CompletableFuture<List<VulnFinding>>> futures = new ArrayList<>();
        for (SecurityRole role : SecurityRole.ALL) {
            futures.add(CompletableFuture.supplyAsync(
                () -> discoverWithRole(role, url, responseBody, headers), executor));
        }
        return aggregate(futures);
    }

    // ── Verification mode: each role validates findings in its domain ──

    public List<VulnFinding> multiRoleVerify(List<VulnFinding> candidates, String codeContext) {
        List<CompletableFuture<List<VulnFinding>>> futures = new ArrayList<>();
        for (SecurityRole role : SecurityRole.ALL) {
            List<VulnFinding> relevant = filterByRole(candidates, role);
            if (relevant.isEmpty()) {
                futures.add(CompletableFuture.completedFuture(List.of()));
                continue;
            }
            futures.add(CompletableFuture.supplyAsync(
                () -> verifyWithRole(role, relevant, codeContext), executor));
        }
        return aggregate(futures);
    }

    // ── Per-role LLM calls ──

    private List<VulnFinding> discoverWithRole(SecurityRole role, String url,
                                                String body, Map<String, List<String>> headers) {
        try {
            String prompt = buildDiscoveryPrompt(role, url, body, headers);
            String response = sendRequest(role.systemPrompt(), prompt, role.maxTokens());
            return parseFindingsResponse(response);
        } catch (Exception e) {
            log.warn("Role {} discovery failed: {}", role.id(), e.getMessage());
            return List.of();
        }
    }

    private List<VulnFinding> verifyWithRole(SecurityRole role,
                                              List<VulnFinding> candidates, String codeContext) {
        try {
            String prompt = buildVerificationPrompt(role, candidates, codeContext);
            String response = sendRequest(role.systemPrompt(), prompt, role.maxTokens());
            return parseVerificationResponse(response, candidates);
        } catch (Exception e) {
            log.warn("Role {} verification failed: {}", role.id(), e.getMessage());
            return new ArrayList<>(candidates);
        }
    }

    // ── Prompt builders ──

    private String buildDiscoveryPrompt(SecurityRole role, String url,
                                         String body, Map<String, List<String>> headers) {
        StringBuilder headerStr = new StringBuilder();
        if (headers != null) {
            headers.forEach((k, v) -> headerStr.append(k).append(": ")
                .append(String.join(",", v)).append("\n"));
        }
        return String.format("""
            分析以下HTTP响应，发现%s相关的安全漏洞。

            URL: %s
            响应头:
            ```
            %s
            ```
            响应体:
            ```
            %s
            ```

            对每个发现的漏洞，返回JSON:
            {"findings":[{"severity":"CRITICAL|HIGH|MEDIUM|LOW","type":"漏洞类型","description":"中文描述","suggestion":"中文修复建议"}]}
            无发现则返回: {"findings":[]}
            """, role.name(), url, headerStr, truncate(body, 3000));
    }

    private String buildVerificationPrompt(SecurityRole role,
                                            List<VulnFinding> candidates, String codeContext) {
        StringBuilder sb = new StringBuilder();
        sb.append("代码上下文:\n```\n").append(truncate(codeContext, 2000)).append("\n```\n\n");
        sb.append("待验证的候选漏洞（仅关注").append(role.name()).append("相关类型）:\n");
        for (int i = 0; i < candidates.size(); i++) {
            VulnFinding f = candidates.get(i);
            sb.append(i + 1).append(". [").append(f.getSeverity()).append("] ")
              .append(f.getType()).append(" @ ").append(f.getLocation())
              .append(" — ").append(truncate(f.getDescription(), 200)).append("\n");
        }
        sb.append("\n对每个候选判断REAL或FALSE，若为REAL则给出修正后的severity/description/suggestion。");
        sb.append("\n返回JSON: {\"results\":[{\"index\":1,\"verdict\":\"REAL|FALSE\",\"severity\":\"...\",\"description\":\"...\",\"suggestion\":\"...\"}]}");
        return sb.toString();
    }

    // ── Response parsers ──

    private List<VulnFinding> parseFindingsResponse(String llmResponse) {
        List<VulnFinding> findings = new ArrayList<>();
        try {
            JsonNode root = mapper.readTree(llmResponse);
            JsonNode arr = root.path("findings");
            if (!arr.isArray()) return findings;
            for (JsonNode f : arr) {
                VulnFinding vf = new VulnFinding();
                vf.setType(f.path("type").asText("LLM发现"));
                vf.setSeverity(f.path("severity").asText("MEDIUM"));
                vf.setDescription(f.path("description").asText(""));
                vf.setSuggestion(f.path("suggestion").asText(""));
                if (!vf.getDescription().isEmpty()) findings.add(vf);
            }
        } catch (Exception e) {
            log.warn("Failed to parse multi-role discovery response");
        }
        return findings;
    }

    private List<VulnFinding> parseVerificationResponse(String llmResponse,
                                                          List<VulnFinding> originals) {
        try {
            JsonNode root = mapper.readTree(llmResponse);
            JsonNode arr = root.path("results");
            if (!arr.isArray()) return new ArrayList<>(originals);
            List<VulnFinding> verified = new ArrayList<>();
            for (JsonNode r : arr) {
                int idx = r.path("index").asInt(-1);
                if (idx < 1 || idx > originals.size()) continue;
                VulnFinding orig = originals.get(idx - 1);
                if ("FALSE".equalsIgnoreCase(r.path("verdict").asText("REAL"))) continue;
                VulnFinding enriched = new VulnFinding();
                enriched.setJobId(orig.getJobId());
                enriched.setType(orig.getType());
                enriched.setLocation(orig.getLocation());
                String sev = r.path("severity").asText("");
                enriched.setSeverity(sev.isEmpty() ? orig.getSeverity() : sev);
                String desc = r.path("description").asText("");
                enriched.setDescription(desc.isEmpty() ? orig.getDescription() : desc);
                String sug = r.path("suggestion").asText("");
                enriched.setSuggestion(sug.isEmpty() ? orig.getSuggestion() : sug);
                verified.add(enriched);
            }
            return verified;
        } catch (Exception e) {
            log.warn("Failed to parse multi-role verification response");
            return new ArrayList<>(originals);
        }
    }

    // ── Aggregation ──

    private List<VulnFinding> aggregate(List<CompletableFuture<List<VulnFinding>>> futures) {
        List<VulnFinding> all = new ArrayList<>();
        for (CompletableFuture<List<VulnFinding>> future : futures) {
            try {
                List<VulnFinding> result = future.get(35, TimeUnit.SECONDS);
                if (result != null) all.addAll(result);
            } catch (Exception e) {
                log.warn("Role timeout or error: {}", e.getMessage());
            }
        }
        return dedupeAndSort(all);
    }

    private List<VulnFinding> dedupeAndSort(List<VulnFinding> findings) {
        Map<String, VulnFinding> deduped = new LinkedHashMap<>();
        for (VulnFinding f : findings) {
            String key = (f.getType() != null ? f.getType() : "") + "|"
                       + (f.getLocation() != null ? f.getLocation() : "");
            VulnFinding existing = deduped.get(key);
            if (existing == null || severityWeight(f.getSeverity()) > severityWeight(existing.getSeverity())) {
                deduped.put(key, f);
            }
        }
        List<VulnFinding> sorted = new ArrayList<>(deduped.values());
        sorted.sort((a, b) -> Integer.compare(severityWeight(b.getSeverity()), severityWeight(a.getSeverity())));
        return sorted;
    }

    private static int severityWeight(String s) {
        return switch (s != null ? s.toUpperCase() : "") {
            case "CRITICAL" -> 4;
            case "HIGH" -> 3;
            case "MEDIUM" -> 2;
            case "LOW" -> 1;
            default -> 0;
        };
    }

    // ── Helpers ──

    private List<VulnFinding> filterByRole(List<VulnFinding> candidates, SecurityRole role) {
        return candidates.stream()
            .filter(f -> role.focusTypes().contains(f.getType()))
            .toList();
    }

    private String sendRequest(String systemPrompt, String userPrompt, int maxTokens)
            throws IOException, InterruptedException {
        ObjectNode body = mapper.createObjectNode();
        body.put("model", model);
        body.put("temperature", 0.1);
        body.put("max_tokens", maxTokens);

        ArrayNode messages = mapper.createArrayNode();
        ObjectNode systemMsg = mapper.createObjectNode();
        systemMsg.put("role", "system");
        systemMsg.put("content", systemPrompt);
        messages.add(systemMsg);

        ObjectNode userMsg = mapper.createObjectNode();
        userMsg.put("role", "user");
        userMsg.put("content", userPrompt);
        messages.add(userMsg);

        body.set("messages", messages);

        String endpoint = baseUrl + (baseUrl.endsWith("/") ? "" : "/") + "chat/completions";
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(endpoint))
                .header("Content-Type", "application/json")
                .header("Authorization", "Bearer " + apiKey)
                .timeout(REQUEST_TIMEOUT)
                .POST(HttpRequest.BodyPublishers.ofString(mapper.writeValueAsString(body)))
                .build();

        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() != 200) {
            String errorBody = truncate(response.body(), 200);
            log.error("LLM API error {}: {}", response.statusCode(), errorBody);
            throw new IOException("LLM API returned " + response.statusCode() + ": " + errorBody);
        }

        JsonNode root = mapper.readTree(response.body());
        return root.path("choices").get(0).path("message").path("content").asText();
    }

    private static String truncate(String s, int maxLen) {
        if (s == null) return "";
        return s.length() <= maxLen ? s : s.substring(0, maxLen) + "...";
    }
}
