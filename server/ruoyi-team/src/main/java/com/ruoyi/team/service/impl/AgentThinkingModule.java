package com.ruoyi.team.service.impl;

import java.io.IOException;
import java.net.URI;
import java.net.http.*;
import java.time.Duration;
import java.util.*;
import java.util.regex.*;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;

public class AgentThinkingModule {
    private static final Logger log = LoggerFactory.getLogger(AgentThinkingModule.class);
    private static final ObjectMapper mapper = new ObjectMapper();
    private static final Duration REQUEST_TIMEOUT = Duration.ofSeconds(30);

    private final String baseUrl;
    private final String model;
    private final String apiKey;
    private final HttpClient httpClient;
    private int roleRoundRobin = 0;

    public AgentThinkingModule(String baseUrl, String model, String apiKey) {
        this.baseUrl = normalizeBaseUrl(baseUrl);
        this.model = model;
        this.apiKey = apiKey;
        this.httpClient = HttpClient.newBuilder()
            .connectTimeout(Duration.ofSeconds(10)).build();
    }

    public String situationalAwareness(String observation, AgentMemory memory) {
        SecurityRole role = selectRole(observation);
        String prompt = String.format("""
            你是%s，正在对目标进行主动安全测试。

            ## 本轮观察
            %s

            ## 相关记忆
            %s

            ## 任务
            分析观察到的内容：
            1. 这个端点/页面是什么功能？
            2. 有什么异常或可疑特征？
            3. 是否存在可利用的弱点？

            输出JSON:
            {"pageType":"...","anomalies":["..."],"weakness":"...","confidence":"HIGH|MEDIUM|LOW","suggestedAction":"..."}
            """, role.name(), truncate(observation, 4000),
            memory.findEndpoints(extractUrlFromObservation(observation)).stream()
                .map(e -> e.summary()).reduce("", (a, b) -> a + b + "\n"));

        try {
            String response = sendRequest(role.systemPrompt(), prompt, 600);
            return response;
        } catch (Exception e) {
            log.warn("Situational awareness failed: {}", e.getMessage());
            return "{\"pageType\":\"unknown\",\"anomalies\":[],\"weakness\":\"\",\"confidence\":\"LOW\",\"suggestedAction\":\"http_request to explore\"}";
        }
    }

    public String strategicDecision(AgentMemory memory, int round, int maxRounds,
                                     List<String> recentObservations) {
        StringBuilder expertAdvice = new StringBuilder();
        for (SecurityRole role : SecurityRole.ALL) {
            try {
                String advice = askExpert(role, memory, recentObservations);
                expertAdvice.append("专家").append(role.name()).append(": ").append(advice).append("\n");
            } catch (Exception e) {
                log.warn("Expert {} failed: {}", role.id(), e.getMessage());
            }
        }

        String prompt = String.format("""
            你是渗透测试指挥官，手下有6位安全专家。

            ## 当前战况 (回合 %d/%d)
            %s

            ## 最近观察
            %s

            ## 专家建议
            %s

            ## 可用工具
            1. http_request - 发起HTTP请求
            2. discover_endpoints - 发现隐藏API
            3. test_injection - 注入测试
            4. analyze_auth - 认证边界测试
            5. switch_identity - 切换凭据
            6. search_memory - 检索记忆
            7. verify_vulnerability - 验证漏洞
            8. chain_attack - 构造攻击链

            ## 任务
            综合专家建议，决定下一步行动。优先考虑：
            1. 串联已有发现的攻击
            2. 未覆盖的攻击面
            3. 高价值目标

            输出JSON:
            {"action":"TOOL_NAME","params":{"param1":"value1",...},"reasoning":"为什么选择这个行动"}
            """, round, maxRounds, memory.summarize(),
            recentObservations.isEmpty() ? "无" :
                String.join("\n", recentObservations.subList(
                    Math.max(0, recentObservations.size() - 5), recentObservations.size())),
            expertAdvice.toString());

        try {
            return sendRequest(
                "你是渗透测试指挥官。只输出JSON，不输出其他内容。",
                prompt, 1000);
        } catch (Exception e) {
            log.warn("Strategic decision failed: {}", e.getMessage());
            return "{\"action\":\"search_memory\",\"params\":{},\"reasoning\":\"fallback\"}";
        }
    }

    private String askExpert(SecurityRole role, AgentMemory memory,
                              List<String> observations) throws Exception {
        String prompt = String.format("""
            作为%s，审视以下战况，给出一个具体的攻击建议。

            %s

            最近: %s

            输出: 一句话攻击建议（中文）
            """, role.name(), memory.summarize(),
            observations.isEmpty() ? "无" : observations.get(observations.size() - 1));

        return sendRequest(role.systemPrompt(), prompt, 300);
    }

    private SecurityRole selectRole(String observation) {
        if (observation == null) return SecurityRole.ALL[roleRoundRobin++ % 6];
        String lower = observation.toLowerCase();
        if (containsAny(lower, "sql", "database", "query", "mysql", "oracle", "injection"))
            return SecurityRole.ALL[0];
        if (containsAny(lower, "login", "auth", "token", "jwt", "session", "cookie", "401", "403"))
            return SecurityRole.ALL[1];
        if (containsAny(lower, "error", "stack", "trace", "debug", "exception", "leak", "path"))
            return SecurityRole.ALL[2];
        if (containsAny(lower, "header", "cors", "csp", "cookie", "x-frame", "hsts"))
            return SecurityRole.ALL[3];
        if (containsAny(lower, "script", "html", "javascript", "dom", "xss", "redirect"))
            return SecurityRole.ALL[4];
        if (containsAny(lower, "method", "put", "delete", "trace", "tls", "port"))
            return SecurityRole.ALL[5];
        return SecurityRole.ALL[roleRoundRobin++ % 6];
    }

    private String sendRequest(String systemPrompt, String userPrompt, int maxTokens)
            throws IOException, InterruptedException {
        ObjectNode body = mapper.createObjectNode();
        body.put("model", model);
        body.put("temperature", 0.3);
        body.put("max_tokens", maxTokens);

        ArrayNode messages = mapper.createArrayNode();
        ObjectNode sys = mapper.createObjectNode();
        sys.put("role", "system");
        sys.put("content", systemPrompt);
        messages.add(sys);

        ObjectNode user = mapper.createObjectNode();
        user.put("role", "user");
        user.put("content", userPrompt);
        messages.add(user);

        body.set("messages", messages);

        String endpoint = baseUrl + (baseUrl.endsWith("/") ? "" : "/") + "chat/completions";
        HttpRequest request = HttpRequest.newBuilder()
            .uri(URI.create(endpoint))
            .header("Content-Type", "application/json")
            .header("Authorization", "Bearer " + apiKey)
            .timeout(REQUEST_TIMEOUT)
            .POST(HttpRequest.BodyPublishers.ofString(mapper.writeValueAsString(body)))
            .build();

        HttpResponse<String> resp = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        if (resp.statusCode() != 200) {
            throw new IOException("LLM API error " + resp.statusCode());
        }
        JsonNode root = mapper.readTree(resp.body());
        return root.path("choices").get(0).path("message").path("content").asText();
    }

    private static String normalizeBaseUrl(String url) {
        if (url == null) return null;
        String trimmed = url.trim().replaceAll("/+$", "");
        if (trimmed.endsWith("/v1") || trimmed.endsWith("/v2")) return trimmed;
        return trimmed;
    }

    private static boolean containsAny(String text, String... keywords) {
        for (String kw : keywords) {
            if (text.contains(kw)) return true;
        }
        return false;
    }

    private static String truncate(String s, int max) {
        if (s == null) return "";
        return s.length() <= max ? s : s.substring(0, max) + "...";
    }

    private static final Pattern URL_EXTRACTOR = Pattern.compile("https?://[^\\s,}\"]+");

    private String extractUrlFromObservation(String obs) {
        if (obs == null) return "";
        Matcher m = URL_EXTRACTOR.matcher(obs);
        return m.find() ? m.group() : "";
    }
}
