package com.ruoyi.team.service.impl;

import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import com.ruoyi.team.domain.TeamModelConfig;
import com.ruoyi.team.domain.VulnFinding;

/**
 * LLM-based vulnerability verification service.
 *
 * Phase 2 of the two-phase scanning pipeline: sends regex-matched candidate
 * findings to an LLM for semantic verification, reducing false positives and
 * improving severity assessment and fix suggestions.
 */
public class VulnLlmVerifier {

    private static final Logger log = LoggerFactory.getLogger(VulnLlmVerifier.class);
    private static final ObjectMapper mapper = new ObjectMapper();
    private static final Duration REQUEST_TIMEOUT = Duration.ofSeconds(30);
    private static final Pattern JSON_OBJECT = Pattern.compile("\\{[^{}]*\"verdict\"[^{}]*\\}",
            Pattern.DOTALL);

    private final String baseUrl;
    private final String model;
    private final String apiKey;
    private final HttpClient httpClient;

    public VulnLlmVerifier(String baseUrl, String model, String apiKey) {
        this.baseUrl = normalizeBaseUrl(baseUrl);
        this.model = model;
        this.apiKey = apiKey;
        this.httpClient = HttpClient.newBuilder()
                .connectTimeout(Duration.ofSeconds(10))
                .build();
    }

    public VulnLlmVerifier(TeamModelConfig config) {
        this(config.getBaseUrl(), config.getModel(), config.getApiKeyHash());
    }

    public String getBaseUrl() { return baseUrl; }
    public String getModel() { return model; }
    public String getApiKey() { return apiKey; }

    public void validateConfig() {
        if (baseUrl == null || baseUrl.isBlank()) {
            throw new IllegalArgumentException("部门模型配置有问题：Base URL 为空");
        }
        if (model == null || model.isBlank()) {
            throw new IllegalArgumentException("部门模型配置有问题：模型名称为空");
        }
        if (apiKey == null || apiKey.isBlank()) {
            throw new IllegalArgumentException("部门模型配置有问题：API-Key 为空");
        }
    }

    public void testConnection() throws IOException, InterruptedException {
        validateConfig();
        sendRequest("请回复严格 JSON：{\"verdict\":\"REAL\",\"severity\":\"LOW\",\"description\":\"连接正常\",\"suggestion\":\"无\"}");
    }

    /**
     * Verify a batch of regex-detected findings through LLM semantic analysis.
     * Each finding is individually evaluated; those confirmed as real
     * vulnerabilities get improved severity/description/suggestion.
     *
     * @param candidates  findings from Phase 1 regex scanning
     * @param codeContext the source code snippet or HTML that triggered the match (max 2000 chars)
     * @return verified and enriched findings
     */
    public List<VulnFinding> verify(List<VulnFinding> candidates, String codeContext) {
        if (candidates.isEmpty()) {
            return candidates;
        }

        List<VulnFinding> verified = new ArrayList<>();
        String truncatedContext = truncate(codeContext, 2000);

        for (VulnFinding finding : candidates) {
            try {
                String response = callLlm(finding, truncatedContext);
                VulnFinding enriched = parseResponse(response, finding);
                if (enriched != null) {
                    verified.add(enriched);
                }
            } catch (Exception e) {
                log.warn("LLM verification failed for {} at {}: {}",
                        finding.getType(), finding.getLocation(), e.getMessage());
                verified.add(finding); // fallback to original
            }
        }
        return verified;
    }

    /**
     * Quick batch classification: given a candidate finding and context,
     * return true if the LLM believes this is a real vulnerability
     * (not a false positive like a comment, log statement, or test mock).
     */
    public boolean isRealVulnerability(VulnFinding finding, String codeContext) {
        try {
            String response = callQuickCheck(finding, truncate(codeContext, 1200));
            return parseQuickCheck(response);
        } catch (Exception e) {
            log.warn("LLM quick check failed for {}: {}", finding.getLocation(), e.getMessage());
            return true; // when in doubt, keep the finding
        }
    }

    private String callLlm(VulnFinding finding, String codeContext) throws IOException, InterruptedException {
        String prompt = buildVerificationPrompt(finding, codeContext);
        return sendRequest(prompt);
    }

    private String callQuickCheck(VulnFinding finding, String codeContext) throws IOException, InterruptedException {
        String prompt = buildQuickCheckPrompt(finding, codeContext);
        return sendRequest(prompt);
    }

    private String sendRequest(String prompt) throws IOException, InterruptedException {
        ObjectNode body = mapper.createObjectNode();
        body.put("model", model);
        body.put("temperature", 0.1);
        body.put("max_tokens", 800);

        ArrayNode messages = mapper.createArrayNode();
        ObjectNode systemMsg = mapper.createObjectNode();
        systemMsg.put("role", "system");
        systemMsg.put("content", "You are a security code reviewer. Respond with JSON only.");
        messages.add(systemMsg);

        ObjectNode userMsg = mapper.createObjectNode();
        userMsg.put("role", "user");
        userMsg.put("content", prompt);
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

    // ---- Prompt builders ----

    private static String buildVerificationPrompt(VulnFinding finding, String codeContext) {
        return String.format("""
                Analyze this code for a potential security vulnerability.

                CODE CONTEXT:
                ```
                %s
                ```

                DETECTED PATTERN: %s at %s
                RULE SEVERITY: %s
                REGEX MATCH: %s

                Determine:
                1. Is this a REAL vulnerability or FALSE POSITIVE (e.g., in a comment, log, test, or safe context)?
                2. If real, what is the CORRECT severity? (CRITICAL / HIGH / MEDIUM / LOW / FALSE)
                3. A concise description of the actual risk (in Chinese)
                4. A specific, actionable fix suggestion (in Chinese, with code example if helpful)

                Respond in JSON ONLY:
                {"verdict": "REAL|FALSE", "severity": "CRITICAL|HIGH|MEDIUM|LOW", "description": "...", "suggestion": "..."}""",
                codeContext,
                finding.getType(),
                finding.getLocation(),
                finding.getSeverity(),
                truncate(finding.getDescription(), 200));
    }

    private static String buildQuickCheckPrompt(VulnFinding finding, String codeContext) {
        return String.format("""
                Is this code pattern a real security vulnerability or a false positive?

                CODE:
                ```
                %s
                ```

                FINDING: %s | Location: %s | Pattern: %s

                Respond: {"verdict":"REAL"} or {"verdict":"FALSE"}""",
                codeContext,
                finding.getType(),
                finding.getLocation(),
                truncate(finding.getDescription(), 150));
    }

    // ---- Response parsers ----

    private static VulnFinding parseResponse(String llmResponse, VulnFinding original) {
        Matcher m = JSON_OBJECT.matcher(llmResponse);
        if (!m.find()) {
            return original;
        }

        try {
            JsonNode json = mapper.readTree(m.group());
            String verdict = json.path("verdict").asText("REAL");

            if ("FALSE".equalsIgnoreCase(verdict)) {
                return null; // signal to remove
            }

            VulnFinding enriched = new VulnFinding();
            enriched.setJobId(original.getJobId());
            enriched.setType(original.getType());
            enriched.setLocation(original.getLocation());

            String severity = json.path("severity").asText("");
            enriched.setSeverity(severity.isEmpty() ? original.getSeverity() : severity);

            String desc = json.path("description").asText("");
            enriched.setDescription(desc.isEmpty() ? original.getDescription() : desc);

            String suggestion = json.path("suggestion").asText("");
            enriched.setSuggestion(suggestion.isEmpty() ? original.getSuggestion() : suggestion);

            return enriched;
        } catch (Exception e) {
            log.warn("Failed to parse LLM response: {}", e.getMessage());
            return original;
        }
    }

    private static boolean parseQuickCheck(String llmResponse) {
        Matcher m = JSON_OBJECT.matcher(llmResponse);
        if (m.find()) {
            try {
                JsonNode json = mapper.readTree(m.group());
                return !"FALSE".equalsIgnoreCase(json.path("verdict").asText("REAL"));
            } catch (Exception ignored) {
            }
        }
        return true; // ambiguous response → keep
    }

    // ---- Helpers ----

    private static String normalizeBaseUrl(String url) {
        if (url == null || url.isBlank()) {
            return "https://api.openai.com/v1";
        }
        String trimmed = url.trim().replaceAll("/+$", "");
        if (trimmed.endsWith("/v1")) {
            return trimmed;
        }
        if (trimmed.contains("/v1")) {
            return trimmed;
        }
        return trimmed + "/v1";
    }

    private static String truncate(String s, int maxLen) {
        if (s == null) return "";
        return s.length() <= maxLen ? s : s.substring(0, maxLen) + "...";
    }

    /**
     * Multi-role discovery: 6 specialized security personas independently
     * analyze the HTTP response from different angles, then aggregate findings.
     */
    public List<VulnFinding> multiRoleDiscover(String url, String responseBody,
                                                Map<String, List<String>> headers) {
        MultiRoleAnalyzer analyzer = new MultiRoleAnalyzer(baseUrl, model, apiKey, httpClient);
        return analyzer.multiRoleDiscover(url, responseBody, headers);
    }

    /**
     * Multi-role verification: filter candidates by role domain, each role
     * independently validates relevant findings in parallel.
     */
    public List<VulnFinding> multiRoleVerify(List<VulnFinding> candidates, String codeContext) {
        MultiRoleAnalyzer analyzer = new MultiRoleAnalyzer(baseUrl, model, apiKey, httpClient);
        return analyzer.multiRoleVerify(candidates, codeContext);
    }

    /**
     * LLM-driven vulnerability discovery: analyze raw HTTP response body
     * to find security issues that rule-based scanning may miss, such as
     * error stack traces, debug info, configuration leaks, and subtle injection patterns.
     */
    public List<VulnFinding> discover(String url, String responseBody, Map<String, List<String>> headers) {
        if (responseBody == null || responseBody.isBlank()) return List.of();
        String truncatedBody = truncate(responseBody, 3000);
        StringBuilder headerStr = new StringBuilder();
        if (headers != null) {
            headers.forEach((k, v) -> headerStr.append(k).append(": ").append(String.join(",", v)).append("\n"));
        }
        try {
            String prompt = buildDiscoveryPrompt(url, truncatedBody, headerStr.toString());
            String llmResponse = sendRequestDiscovery(prompt);
            return parseDiscoveryResponse(llmResponse);
        } catch (Exception e) {
            log.warn("LLM discovery failed for {}: {}", url, e.getMessage());
            return List.of();
        }
    }

    private String buildDiscoveryPrompt(String url, String body, String headers) {
        return String.format("""
                You are a security expert. Analyze this HTTP response for security vulnerabilities.

                URL: %s

                RESPONSE HEADERS:
                ```
                %s
                ```

                RESPONSE BODY:
                ```
                %s
                ```

                Look for:
                1. Error messages leaking stack traces, file paths, SQL queries, or internal IPs
                2. Debug endpoints or debug mode indicators (debug=true, X-Debug-Token, Symfony profiler, etc.)
                3. Exposed configuration values (database URLs, API keys, secrets in responses)
                4. Version disclosure (server, framework, library versions in headers or body)
                5. Unusual error responses that suggest backend behavior (NullPointer, TypeError, etc.)
                6. Missing security headers that should be present
                7. Any other suspicious patterns or information leaks

                If you find vulnerabilities, respond in JSON:
                {"findings": [{"severity": "CRITICAL|HIGH|MEDIUM|LOW", "type": "...", "description": "...", "suggestion": "..."}]}

                If no issues found, respond: {"findings": []}
                """, url, headers, body);
    }

    private String sendRequestDiscovery(String prompt) throws IOException, InterruptedException {
        ObjectNode body = mapper.createObjectNode();
        body.put("model", model);
        body.put("temperature", 0.1);
        body.put("max_tokens", 1200);

        ArrayNode messages = mapper.createArrayNode();
        ObjectNode systemMsg = mapper.createObjectNode();
        systemMsg.put("role", "system");
        systemMsg.put("content", "You are a security expert. Respond with JSON only.");
        messages.add(systemMsg);

        ObjectNode userMsg = mapper.createObjectNode();
        userMsg.put("role", "user");
        userMsg.put("content", prompt);
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
            throw new IOException("LLM API returned " + response.statusCode());
        }
        JsonNode root = mapper.readTree(response.body());
        return root.path("choices").get(0).path("message").path("content").asText();
    }

    private List<VulnFinding> parseDiscoveryResponse(String llmResponse) {
        List<VulnFinding> findings = new ArrayList<>();
        try {
            JsonNode root = mapper.readTree(llmResponse);
            JsonNode findingsNode = root.path("findings");
            if (!findingsNode.isArray()) return findings;
            for (JsonNode f : findingsNode) {
                VulnFinding finding = new VulnFinding();
                finding.setType(f.path("type").asText("LLM发现"));
                finding.setSeverity(f.path("severity").asText("MEDIUM"));
                finding.setDescription(f.path("description").asText(""));
                finding.setSuggestion(f.path("suggestion").asText(""));
                if (!finding.getDescription().isEmpty()) {
                    findings.add(finding);
                }
            }
        } catch (Exception e) {
            log.warn("Failed to parse LLM discovery response: {}", e.getMessage());
        }
        return findings;
    }
}
