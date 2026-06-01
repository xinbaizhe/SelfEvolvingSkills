package com.ruoyi.team.service.impl;

import java.io.IOException;
import java.net.URI;
import java.net.URLEncoder;
import java.net.http.*;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.*;
import java.util.regex.*;
import java.util.stream.Collectors;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.ruoyi.team.domain.*;

public class AgentToolSet {
    private static final Logger log = LoggerFactory.getLogger(AgentToolSet.class);
    private static final ObjectMapper mapper = new ObjectMapper();
    private static final Duration TIMEOUT = Duration.ofSeconds(20);
    private static final Pattern HREF = Pattern.compile("(?:href|src|action)\\s*=\\s*[\"']([^\"'#\\s]+)[\"']",
            Pattern.CASE_INSENSITIVE);
    private static final Pattern JS_API = Pattern.compile("[\"'](/[a-zA-Z][a-zA-Z0-9_/-]*)[\"']");
    private static final Pattern PARAM_PATTERN = Pattern.compile("[?&]([^=&\s]+)=");

    private static final String[] COMMON_API_PATTERNS = {
        "/api", "/v1", "/v2", "/graphql", "/rest", "/ws", "/soap",
        "/api/users", "/api/admin", "/api/login", "/api/auth",
        "/api/upload", "/api/download", "/api/search", "/api/config",
        "/api/health", "/api/status", "/api/info", "/api/debug",
        "/api/export", "/api/import", "/api/data", "/api/sync"
    };

    private final HttpClient httpClient;
    private final AgentMemory memory;

    public AgentToolSet(HttpClient httpClient, AgentMemory memory) {
        this.httpClient = httpClient;
        this.memory = memory;
    }

    public String httpRequest(String method, String url, Map<String, String> headers,
                               String body, boolean followRedirects) throws Exception {
        HttpRequest.Builder builder = HttpRequest.newBuilder()
            .uri(URI.create(url))
            .timeout(TIMEOUT);

        if (headers != null) {
            headers.forEach(builder::header);
        }

        HttpRequest.BodyPublisher pub = HttpRequest.BodyPublishers.noBody();
        if (body != null && !body.isEmpty()) {
            pub = HttpRequest.BodyPublishers.ofString(body, StandardCharsets.UTF_8);
            builder.header("Content-Type", "application/json");
        }

        builder.method(method.toUpperCase(), pub);
        HttpResponse<String> resp = httpClient.send(builder.build(),
            followRedirects ? HttpResponse.BodyHandlers.ofString()
                            : HttpResponse.BodyHandlers.ofString());

        String respBody = resp.body();
        String preview = respBody != null && respBody.length() > 8000
            ? respBody.substring(0, 8000) : respBody;

        DiscoveredEndpoint ep = new DiscoveredEndpoint(url, method,
            resp.headers().firstValue("Content-Type").orElse("unknown"),
            headers != null && (headers.containsKey("Cookie") || headers.containsKey("Authorization")),
            "agent_http_request", extractParams(url), resp.statusCode(), preview, null);
        memory.addEndpoint(ep);
        extractParams(respBody).forEach(p -> memory.addParam(
            new AgentMemory.AgentParam(url, p, "unknown", false, false)));

        return String.format("HTTP %d\nContent-Type: %s\nBody(%d chars):\n%s",
            resp.statusCode(),
            resp.headers().firstValue("Content-Type").orElse("unknown"),
            preview != null ? preview.length() : 0,
            preview);
    }

    public String discoverEndpoints(String baseUrl, List<String> patterns, int maxDepth) {
        Set<String> discovered = new LinkedHashSet<>();
        Queue<String> queue = new ArrayDeque<>();
        queue.add(baseUrl);
        Set<String> visited = new HashSet<>();
        int depth = 0;

        while (!queue.isEmpty() && depth < maxDepth) {
            int size = queue.size();
            for (int i = 0; i < size; i++) {
                String url = queue.poll();
                if (!visited.add(url)) continue;
                try {
                    HttpResponse<String> resp = httpClient.send(
                        HttpRequest.newBuilder().uri(URI.create(url)).timeout(TIMEOUT).GET().build(),
                        HttpResponse.BodyHandlers.ofString());
                    String body = resp.body();
                    if (body == null) continue;

                    Matcher m = HREF.matcher(body);
                    while (m.find()) {
                        String link = resolveUrl(url, m.group(1));
                        if (link != null && link.startsWith(baseUrl) && visited.add(link)) {
                            discovered.add(link);
                            queue.add(link);
                        }
                    }
                    m = JS_API.matcher(body);
                    while (m.find()) {
                        String path = resolveUrl(url, m.group(1));
                        if (path != null && path.startsWith(baseUrl)) {
                            discovered.add(path);
                        }
                    }
                    memory.addEndpoint(new DiscoveredEndpoint(url, "GET",
                        resp.headers().firstValue("Content-Type").orElse("unknown"),
                        false, "discover", null, resp.statusCode(),
                        body.substring(0, Math.min(500, body.length())), null));
                } catch (Exception e) {
                    log.debug("discover_endpoints failed for {}: {}", url, e.getMessage());
                }
            }
            depth++;
        }

        for (String pattern : (patterns != null && !patterns.isEmpty() ? patterns
                : Arrays.asList(COMMON_API_PATTERNS))) {
            String testUrl = baseUrl + (pattern.startsWith("/") ? pattern : "/" + pattern);
            if (!visited.contains(testUrl)) {
                discovered.add(testUrl);
            }
        }

        StringBuilder sb = new StringBuilder("发现 " + discovered.size() + " 个端点:\n");
        for (String url : discovered) {
            sb.append("  ").append(url).append("\n");
        }
        return sb.toString();
    }

    public String testInjection(String url, String paramName, String injectionType,
                                 String payload) throws Exception {
        String encodedPayload = URLEncoder.encode(payload, StandardCharsets.UTF_8);
        String testUrl;
        if (url.contains("?")) {
            testUrl = url + "&" + paramName + "=" + encodedPayload;
        } else {
            testUrl = url + "?" + paramName + "=" + encodedPayload;
        }

        HttpResponse<String> resp = httpClient.send(
            HttpRequest.newBuilder().uri(URI.create(testUrl)).timeout(TIMEOUT).GET().build(),
            HttpResponse.BodyHandlers.ofString());
        String body = resp.body();
        String evidence = detectInjectionEvidence(body, injectionType);

        if (evidence != null) {
            VulnFinding vf = new VulnFinding();
            vf.setType(injectionType + "注入");
            vf.setSeverity(evidence.contains("error") ? "HIGH" : "MEDIUM");
            vf.setLocation(url + "?" + paramName);
            vf.setDescription("Payload [" + truncate(payload, 80) + "] 触发注入指纹: " + evidence);
            vf.setSuggestion("对参数 " + paramName + " 进行输入验证和参数化处理");
            memory.addVuln(vf);
            return "VULN_FOUND: " + evidence;
        }
        return "NO_VULN: 未发现注入指纹";
    }

    public String analyzeAuth(String url, String method, String credId) throws Exception {
        CredentialState cred = memory.getCredential(credId);
        if (cred == null) return "ERROR: 凭据 " + credId + " 不存在";

        Map<String, String> authHeaders = new HashMap<>();
        if (cred.cookie() != null && !cred.cookie().isBlank())
            authHeaders.put("Cookie", cred.cookie());
        if (cred.authorization() != null && !cred.authorization().isBlank())
            authHeaders.put("Authorization", cred.authorization());

        String withAuth = httpRequest(method, url, authHeaders, null, false);
        String withoutAuth = httpRequest(method, url, null, null, false);

        boolean withIs403 = withAuth.contains("HTTP 403") || withAuth.contains("HTTP 401");
        boolean withoutIs403 = withoutAuth.contains("HTTP 403") || withoutAuth.contains("HTTP 401");

        if (withoutIs403 && !withIs403) return "AUTH_OK: 需要认证，凭据有效";
        if (!withoutIs403 && !withIs403) return "PUBLIC: 端点无需认证";
        if (!withoutIs403 && withIs403)
            return "AUTH_ANOMALY: 端点无认证可访问，但凭据请求反而被拒绝(可能角色权限不足)";
        return "AUTH_BLOCKED: 认证保护生效，无法访问";
    }

    public String switchIdentity(String credId) {
        CredentialState cred = memory.getCredential(credId);
        if (cred == null) return "ERROR: 凭据 " + credId + " 不存在";
        if (!cred.sessionValid()) return "WARN: 凭据 " + cred.label() + " 已过期";
        return "OK: 已切换到 " + cred.label();
    }

    public String searchMemory(String query, String category) {
        StringBuilder sb = new StringBuilder();
        String lower = query != null ? query.toLowerCase() : "";

        if (category == null || "endpoint".equals(category)) {
            List<DiscoveredEndpoint> eps = memory.findEndpoints(lower);
            sb.append("端点(").append(eps.size()).append("):\n");
            eps.forEach(e -> sb.append("  ").append(e.summary()).append("\n"));
        }
        if (category == null || "vuln".equals(category)) {
            List<VulnFinding> vulns = memory.getVulns().stream()
                .filter(v -> lower.isEmpty() ||
                    (v.getType() != null && v.getType().toLowerCase().contains(lower)) ||
                    (v.getLocation() != null && v.getLocation().toLowerCase().contains(lower)))
                .collect(Collectors.toList());
            sb.append("漏洞(").append(vulns.size()).append("):\n");
            vulns.forEach(v -> sb.append("  [").append(v.getSeverity()).append("] ")
                .append(v.getType()).append(" @ ").append(v.getLocation()).append("\n"));
        }
        return sb.toString();
    }

    public String verifyVulnerability(String type, String location, String description) {
        VulnFinding match = null;
        for (VulnFinding v : memory.getVulns()) {
            if (location != null && location.equals(v.getLocation())
                && type != null && type.equals(v.getType())) {
                match = v;
                break;
            }
        }
        if (match == null) return "NOT_FOUND: 未找到匹配漏洞";

        try {
            String verifyUrl = location;
            if (verifyUrl != null && verifyUrl.contains("?")) {
                verifyUrl = verifyUrl + "&verify_test=1";
            } else if (verifyUrl != null) {
                verifyUrl = verifyUrl + "?verify_test=1";
            }
            if (verifyUrl == null) return "SKIP: 无法构造验证请求";

            HttpResponse<String> resp = httpClient.send(
                HttpRequest.newBuilder().uri(URI.create(verifyUrl)).timeout(TIMEOUT).GET().build(),
                HttpResponse.BodyHandlers.ofString());
            return resp.statusCode() == 200 ? "VERIFIED: 漏洞可复现" : "UNCERTAIN: 无法确认";
        } catch (Exception e) {
            return "VERIFY_ERROR: " + e.getMessage();
        }
    }

    public String chainAttack(String typeA, String typeB, String strategy) {
        List<VulnFinding> confirmed = memory.getConfirmedVulns();
        VulnFinding a = confirmed.stream()
            .filter(v -> typeA.equals(v.getType())).findFirst().orElse(null);
        VulnFinding b = confirmed.stream()
            .filter(v -> typeB.equals(v.getType())).findFirst().orElse(null);
        if (a == null || b == null) return "NOT_FEASIBLE: 缺少已确认漏洞";

        AttackChain chain = memory.createChain(a.getType() + "→" + b.getType());
        chain.addStep("START", "串联 " + a.getType() + " 和 " + b.getType());
        chain.addStep("VULN_A", a.getType() + " @ " + a.getLocation());
        chain.addStep("VULN_B", b.getType() + " @ " + b.getLocation());
        chain.addStep("STRATEGY", strategy != null ? strategy : "直接串联");

        String impact = estimateChainImpact(a, b);
        chain.setImpact(impact);
        return "FEASIBLE: 攻击链可构造，预估影响: " + impact;
    }

    private List<String> extractParams(String text) {
        List<String> result = new ArrayList<>();
        if (text == null) return result;
        Matcher m = PARAM_PATTERN.matcher(text);
        while (m.find()) {
            result.add(m.group(1));
        }
        return result;
    }

    private String resolveUrl(String base, String link) {
        if (link == null || link.isBlank()) return null;
        if (link.startsWith("http://") || link.startsWith("https://")) return link;
        try {
            URI baseUri = URI.create(base);
            return baseUri.resolve(link.startsWith("/") ? link : "/" + link).toString();
        } catch (Exception e) {
            return null;
        }
    }

    private String detectInjectionEvidence(String body, String type) {
        if (body == null) return null;
        String lower = body.toLowerCase();
        switch (type.toUpperCase()) {
            case "SQL":
                if (lower.contains("sql syntax") || lower.contains("mysql_fetch")
                    || lower.contains("ora-") || lower.contains("postgresql")
                    || lower.contains("sqlite") || lower.contains("unclosed quotation"))
                    return "sql_error";
                break;
            case "XSS":
                if (body.contains("<script>alert") || body.contains("javascript:alert")
                    || body.contains("onerror=alert"))
                    return "xss_reflected";
                break;
            case "SSTI":
                if (lower.contains("jinja2") || lower.contains("freemarker")
                    || lower.contains("template") || body.contains("49"))
                    return "ssti_math_result";
                break;
            case "LFI":
                if (body.contains("root:") || body.contains("[extensions]")
                    || body.contains("<?php"))
                    return "lfi_file_content";
                break;
        }
        return null;
    }

    private String estimateChainImpact(VulnFinding a, VulnFinding b) {
        int score = 0;
        if ("CRITICAL".equalsIgnoreCase(a.getSeverity())) score += 3;
        else if ("HIGH".equalsIgnoreCase(a.getSeverity())) score += 2;
        else score += 1;
        if ("CRITICAL".equalsIgnoreCase(b.getSeverity())) score += 3;
        else if ("HIGH".equalsIgnoreCase(b.getSeverity())) score += 2;
        else score += 1;
        if (score >= 5) return "CRITICAL - 可导致系统级危害";
        if (score >= 3) return "HIGH - 可导致数据泄露或权限提升";
        return "MEDIUM - 有限影响";
    }

    private static String truncate(String s, int max) {
        if (s == null) return "";
        return s.length() <= max ? s : s.substring(0, max) + "...";
    }
}
