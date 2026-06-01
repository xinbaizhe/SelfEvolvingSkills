package com.ruoyi.team.service.impl;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.net.HttpURLConnection;
import java.net.URI;
import java.net.URL;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.Duration;
import java.time.LocalDateTime;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import java.util.function.Consumer;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import com.ruoyi.team.domain.CredentialState;
import com.ruoyi.team.domain.TeamModelConfig;
import com.ruoyi.team.domain.VulnFinding;
import com.ruoyi.team.domain.VulnScanJob;
import com.ruoyi.team.mapper.VulnFindingMapper;
import com.ruoyi.team.mapper.VulnScanJobMapper;
import com.ruoyi.team.service.ITeamModelConfigService;
import com.ruoyi.team.service.IVulnScanService;

/**
 * Two-phase hybrid vulnerability scanning service.
 *
 * Phase 1 — Fast regex pattern matching (high recall):
 * Scans code/HTML with regex rules to catch all potential issues.
 *
 * Phase 2 — LLM semantic verification (high precision):
 * Sends each regex match to an LLM to filter false positives and enrich
 * severity/description/suggestion. Falls back to regex-only when no LLM
 * config is available.
 */
@Service
public class VulnScanServiceImpl implements IVulnScanService {

    private static final Logger log = LoggerFactory.getLogger(VulnScanServiceImpl.class);

    @Autowired
    private VulnScanJobMapper jobMapper;

    @Autowired
    private VulnFindingMapper findingMapper;

    @Autowired
    private ITeamModelConfigService modelConfigService;

    // Regex patterns — Phase 1 fast detectors
    private static final Pattern FORM_PATTERN = Pattern.compile(
        "<form[^>]*method\\s*=\\s*[\"']?post[\"']?[^>]*>", Pattern.CASE_INSENSITIVE);
    private static final Pattern INPUT_PATTERN = Pattern.compile(
        "<input[^>]*>", Pattern.CASE_INSENSITIVE);
    private static final Pattern SCRIPT_PATTERN = Pattern.compile(
        "<script[^>]*>", Pattern.CASE_INSENSITIVE);
    private static final Pattern INLINE_SCRIPT_PATTERN = Pattern.compile(
        "on(?:error|load|click|submit|focus|blur|change|mouseover|keyup|keydown)\\s*=",
        Pattern.CASE_INSENSITIVE);
    private static final Pattern HREF_JS_PATTERN = Pattern.compile(
        "href\\s*=\\s*[\"']\\s*javascript:", Pattern.CASE_INSENSITIVE);
    private static final Pattern REDIRECT_PARAM_PATTERN = Pattern.compile(
        "[?&](?:redirect|url|next|return|returnUrl|goto|target|redir|forward|callback|dest|destination)\\s*=",
        Pattern.CASE_INSENSITIVE);

    private static final List<Rule> CODE_RULES = Arrays.asList(
        new Rule("硬编码密钥", "CRITICAL", "发现疑似硬编码的密钥或凭证",
            "请立即将密钥移至环境变量或密钥管理服务中，并轮换已泄露的密钥",
            "(?i)(?:api[_-]?key|apikey|secret[_-]?key|secretkey|access[_-]?key)\\s*[:=]\\s*[\"'][A-Za-z0-9_\\-]{20,}[\"']",
            "(?i)(?:password|passwd|pwd)\\s*[:=]\\s*[\"'][^\"']{4,}[\"']",
            "(?i)(?:token|jwt)\\s*[:=]\\s*[\"'][A-Za-z0-9_\\-\\.]{20,}[\"']",
            "(?i)(?:private[_-]?key|privatekey)\\s*[:=]\\s*[\"']-----BEGIN",
            "(?i)(?:connection[_-]?string|connstr)\\s*[:=]\\s*[\"'][^\"']{10,}[\"']",
            "(?i)sk-[A-Za-z0-9]{20,}",
            "(?i)ghp_[A-Za-z0-9]{30,}"),

        new Rule("SQL注入", "CRITICAL", null, "请使用参数化查询（PreparedStatement / ORM参数绑定）替代字符串拼接",
            "(?i)\"(?:SELECT|INSERT|UPDATE|DELETE|DROP)\\s.*\\+\\s*",
            "(?i)'(?:SELECT|INSERT|UPDATE|DELETE|DROP)\\s.*\\+\\s*",
            "(?i)String\\s+\\w+\\s*=\\s*\"\\s*(?:SELECT|INSERT|UPDATE|DELETE)",
            "(?i)(?:execute|executeQuery|executeUpdate)\\s*\\(\\s*\\w+\\s*\\+",
            "(?i)format\\s*\\(\\s*\"\\s*(?:SELECT|INSERT|UPDATE|DELETE)",
            "(?i)f[\"']\\s*(?:SELECT|INSERT|UPDATE|DELETE)"),

        new Rule("XSS", "HIGH", null, "请对用户输入进行HTML转义，使用textContent替代innerHTML",
            "(?i)innerHTML\\s*=",
            "(?i)dangerouslySetInnerHTML",
            "(?i)v-html\\s*=",
            "(?i)document\\.write\\s*\\(",
            "(?i)\\.html\\s*\\(\\s*[^)]*\\$"),

        new Rule("路径遍历", "HIGH", null, "请验证和规范化用户输入的路径，使用白名单限制访问目录",
            "(?i)new\\s+File(?:InputStream|Reader|Writer|OutputStream)?\\s*\\(.*\\+",
            "(?i)Files\\.(?:read|write|copy|move|newInputStream|newOutputStream)\\s*\\(.*\\+",
            "(?i)Paths\\.get\\s*\\(.*\\+"),

        new Rule("命令注入", "CRITICAL", null, "请使用参数数组替代字符串拼接，并对用户输入进行严格过滤",
            "(?i)Runtime\\.getRuntime\\(\\)\\.exec\\s*\\(.*\\+",
            "(?i)ProcessBuilder\\s*\\([^)]*\\+",
            "(?i)exec\\s*\\(\\s*[\"'][^\"']*\\$",
            "(?i)system\\s*\\(\\s*[\"'][^\"']*\\$",
            "(?i)shell_exec\\s*\\(\\s*[\"'][^\"']*\\$",
            "(?i)os\\.system\\s*\\(.*\\+",
            "(?i)subprocess\\.(?:call|Popen|run|check_output)\\s*\\(.*\\+"),

        new Rule("不安全的加密", "MEDIUM", null, "建议使用SHA-256+哈希算法、AES/GCM加密模式",
            "(?i)MessageDigest\\.getInstance\\s*\\(\\s*\"MD5\"",
            "(?i)MessageDigest\\.getInstance\\s*\\(\\s*\"SHA-1\"",
            "(?i)hashlib\\.md5\\s*\\(",
            "(?i)hashlib\\.sha1\\s*\\(",
            "(?i)md5\\s*\\(",
            "(?i)Cipher\\.getInstance\\s*\\(\\s*\"DES\"",
            "(?i)Cipher\\.getInstance\\s*\\(\\s*\"RC4\"",
            "(?i)\"AES/ECB")
    );

    // ---- Rule data class ----

    private static class Rule {
        final String type;
        final String severity;
        final String defaultDescription;
        final String defaultSuggestion;
        final List<Pattern> patterns;

        Rule(String type, String severity, String defaultDescription, String defaultSuggestion,
             String... regexes) {
            this.type = type;
            this.severity = severity;
            this.defaultDescription = defaultDescription;
            this.defaultSuggestion = defaultSuggestion;
            this.patterns = Arrays.stream(regexes).map(Pattern::compile).toList();
        }
    }

    // ---- Public API ----

    @Override
    @Transactional
    public VulnScanJob scanUrl(String targetUrl, Long userId, Long deptId, String modelType, Long modelId) {
        VulnScanJob job = createJob("url", targetUrl, userId, deptId, modelType, modelId);
        jobMapper.insertVulnScanJob(job);

        List<VulnFinding> findings = doUrlScan(targetUrl);
        saveFindings(job.getId(), findings);
        updateJobCounts(job, findings);
        job.setFindings(findings);
        return job;
    }

    @Override
    @Transactional
    public VulnScanJob scanCode(String dirPath, Long userId, Long deptId, String modelType, Long modelId) {
        VulnScanJob job = createJob("code", dirPath, userId, deptId, modelType, modelId);
        jobMapper.insertVulnScanJob(job);

        VulnLlmVerifier llm = buildVerifier(deptId, userId);
        List<VulnFinding> findings = doCodeScan(dirPath, llm);
        saveFindings(job.getId(), findings);
        updateJobCounts(job, findings);
        job.setFindings(findings);
        return job;
    }

    @Override
    public VulnScanJob getJobWithFindings(Long jobId) {
        VulnScanJob job = jobMapper.selectVulnScanJobById(jobId);
        if (job != null) {
            job.setFindings(findingMapper.selectFindingsByJobId(jobId));
        }
        return job;
    }

    @Override
    public List<VulnScanJob> getHistory(Long userId) {
        VulnScanJob query = new VulnScanJob();
        query.setUserId(userId);
        query.setStatus("completed");
        return jobMapper.selectVulnScanJobList(query);
    }

    // ---- LLM verifier setup ----

    private VulnLlmVerifier buildVerifier(Long deptId, Long userId) {
        try {
            List<TeamModelConfig> configs = modelConfigService.selectActiveModelConfigsByDeptId(deptId);
            if (configs.isEmpty()) {
                configs = modelConfigService.selectAvailableModelConfigs(deptId, userId, null);
            }
            if (!configs.isEmpty()) {
                TeamModelConfig config = configs.get(0);
                log.info("Using LLM model {} for vuln verification", config.getModel());
                return new VulnLlmVerifier(config);
            }
        } catch (Exception e) {
            log.warn("Failed to load LLM config for vuln verification: {}", e.getMessage());
        }
        log.info("No LLM config available, falling back to regex-only scanning");
        return null;
    }

    // ---- Job helpers ----

    private VulnScanJob createJob(String type, String target, Long userId, Long deptId,
                                   String modelType, Long modelId) {
        VulnScanJob job = new VulnScanJob();
        job.setScanType(type);
        job.setTarget(target);
        job.setStatus("running");
        job.setUserId(userId);
        job.setDeptId(deptId);
        job.setModelType(modelType);
        job.setModelId(modelId);
        job.setCreatedAt(LocalDateTime.now());
        return job;
    }

    private void saveFindings(Long jobId, List<VulnFinding> findings) {
        if (findings.isEmpty()) return;
        for (VulnFinding f : findings) {
            f.setJobId(jobId);
        }
        findingMapper.batchInsertVulnFindings(findings);
    }

    private void updateJobCounts(VulnScanJob job, List<VulnFinding> findings) {
        job.setTotalFindings(findings.size());
        job.setCriticalCount((int) findings.stream().filter(f -> "CRITICAL".equals(f.getSeverity())).count());
        job.setHighCount((int) findings.stream().filter(f -> "HIGH".equals(f.getSeverity())).count());
        job.setMediumCount((int) findings.stream().filter(f -> "MEDIUM".equals(f.getSeverity())).count());
        job.setLowCount((int) findings.stream().filter(f -> "LOW".equals(f.getSeverity())).count());
        job.setStatus("completed");
        jobMapper.updateVulnScanJob(job);
    }

    // ---- URL scanning (Phase 1 only — header/HTML checks are unambiguous) ----

    private List<VulnFinding> doUrlScan(String targetUrl) {
        List<VulnFinding> findings = new ArrayList<>();
        try {
            String normalizedUrl = targetUrl.startsWith("http") ? targetUrl : "https://" + targetUrl;
            URI uri = URI.create(normalizedUrl);
            URL url = uri.toURL();
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            conn.setRequestMethod("GET");
            conn.setConnectTimeout(10000);
            conn.setReadTimeout(15000);
            conn.setInstanceFollowRedirects(false);
            conn.setRequestProperty("User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) SecurityScanner/2.0");

            int responseCode = conn.getResponseCode();
            Map<String, List<String>> headers = conn.getHeaderFields();

            checkSecurityHeaders(headers, normalizedUrl, findings);

            if (responseCode >= 300 && responseCode < 400) {
                String location = conn.getHeaderField("Location");
                if (location != null) {
                    checkOpenRedirect(targetUrl, location, findings);
                }
            }

            String contentType = conn.getContentType();
            if (contentType != null && contentType.toLowerCase().contains("text/html")) {
                StringBuilder content = new StringBuilder();
                try (BufferedReader reader = new BufferedReader(
                        new InputStreamReader(conn.getInputStream()))) {
                    String line;
                    while ((line = reader.readLine()) != null) {
                        content.append(line).append("\n");
                    }
                }
                scanHtmlContent(content.toString(), normalizedUrl, findings);
            }
            conn.disconnect();
        } catch (Exception e) {
            findings.add(new VulnFinding(null, "MEDIUM", "连接错误", targetUrl,
                "无法连接到目标URL: " + e.getMessage(), "请确认URL是否正确且可访问"));
        }
        return findings;
    }

    private void checkSecurityHeaders(Map<String, List<String>> headers,
                                       String url, List<VulnFinding> findings) {
        Map<String, String> required = Map.of(
            "Content-Security-Policy", "CSP",
            "X-Frame-Options", "X-Frame-Options",
            "X-Content-Type-Options", "X-Content-Type-Options",
            "Strict-Transport-Security", "HSTS"
        );
        for (Map.Entry<String, String> e : required.entrySet()) {
            if (!headers.containsKey(e.getKey())) {
                findings.add(new VulnFinding(null, "MEDIUM", "缺少安全响应头", url,
                    "缺少 " + e.getValue() + " 响应头", "建议添加 " + e.getKey() + " 响应头以增强安全性"));
            }
        }
        List<String> server = headers.get("Server");
        if (server != null) {
            for (String v : server) {
                if (v != null && !v.isBlank()) {
                    findings.add(new VulnFinding(null, "LOW", "信息泄露", url,
                        "Server 响应头泄露了服务器信息: " + v, "建议移除或隐藏 Server 响应头"));
                }
            }
        }
        List<String> poweredBy = headers.get("X-Powered-By");
        if (poweredBy != null && !poweredBy.isEmpty()) {
            findings.add(new VulnFinding(null, "LOW", "信息泄露", url,
                "X-Powered-By 响应头泄露了技术栈信息", "建议移除 X-Powered-By 响应头"));
        }
    }

    private void checkOpenRedirect(String targetUrl, String location, List<VulnFinding> findings) {
        Matcher m = REDIRECT_PARAM_PATTERN.matcher(targetUrl);
        if (m.find()) {
            findings.add(new VulnFinding(null, "HIGH", "开放重定向", targetUrl,
                "URL包含重定向参数，可能被利用进行钓鱼攻击", "建议对重定向目标进行白名单验证"));
        }
    }

    private void scanHtmlContent(String html, String url, List<VulnFinding> findings) {
        Matcher fm = FORM_PATTERN.matcher(html);
        if (fm.find()) {
            String section = html.substring(Math.max(0, fm.start() - 200),
                Math.min(html.length(), fm.end() + 2000));
            boolean hasCsrf = section.toLowerCase().contains("csrf")
                || section.toLowerCase().contains("_token")
                || section.toLowerCase().contains("authenticity_token");
            if (!hasCsrf) {
                findings.add(new VulnFinding(null, "HIGH", "CSRF", url,
                    "表单缺少CSRF防护token", "建议在所有表单中添加CSRF token并在服务端验证"));
            }
            Matcher im = INPUT_PATTERN.matcher(section);
            while (im.find()) {
                String tag = im.group();
                if (tag.toLowerCase().contains("type=\"password\"")
                    && !section.toLowerCase().contains("autocomplete=\"off\"")) {
                    findings.add(new VulnFinding(null, "LOW", "密码自动填充", url,
                        "密码输入框未禁用autocomplete", "建议添加 autocomplete=\"off\" 属性"));
                }
            }
        }
        Matcher sm = SCRIPT_PATTERN.matcher(html);
        int count = 0;
        while (sm.find()) {
            count++;
            String tag = sm.group();
            if (!tag.toLowerCase().contains("nonce=") && !tag.toLowerCase().contains("integrity=")) {
                findings.add(new VulnFinding(null, "MEDIUM", "内联脚本",
                    url + " (脚本#" + count + ")", "script标签缺少nonce或integrity属性",
                    "建议使用CSP nonce或SRI integrity属性"));
                break;
            }
        }
        Matcher ilm = INLINE_SCRIPT_PATTERN.matcher(html);
        if (ilm.find()) {
            findings.add(new VulnFinding(null, "MEDIUM", "内联事件处理器", url,
                "HTML元素使用了内联事件处理器(" + ilm.group() + ")",
                "建议将事件处理移到外部JS文件中"));
        }
        Matcher jm = HREF_JS_PATTERN.matcher(html);
        if (jm.find()) {
            findings.add(new VulnFinding(null, "HIGH", "XSS", url,
                "发现 javascript: 协议链接，可能被用于XSS攻击",
                "建议移除javascript:伪协议，使用事件监听器替代"));
        }
        if (html.toLowerCase().contains("<meta http-equiv=\"refresh")) {
            findings.add(new VulnFinding(null, "LOW", "Meta重定向", url,
                "使用了meta refresh重定向", "建议使用HTTP 301/302重定向替代meta refresh"));
        }
    }

    // ---- Code scanning (Phase 1 regex + Phase 2 LLM) ----

    private List<VulnFinding> doCodeScan(String dirPath, VulnLlmVerifier llm) {
        List<VulnFinding> findings = new ArrayList<>();
        Path root = Paths.get(dirPath);
        if (!Files.exists(root) || !Files.isDirectory(root)) {
            findings.add(new VulnFinding(null, "MEDIUM", "路径错误", dirPath,
                "目录不存在或无法访问", "请确认目录路径是否正确"));
            return findings;
        }

        // Phase 1: regex scan per file, collect with context
        Map<String, List<VulnFinding>> findingsByFile = new HashMap<>();
        Map<String, String> fileContents = new HashMap<>();

        try (Stream<Path> walk = Files.walk(root, 8)) {
            walk.filter(Files::isRegularFile)
                .filter(p -> isSourceFile(p))
                .forEach(fp -> {
                    try {
                        String content = Files.readString(fp);
                        String relPath = root.relativize(fp).toString();
                        fileContents.put(relPath, content);

                        List<VulnFinding> fileFindings = scanFileWithRules(content, relPath);
                        if (!fileFindings.isEmpty()) {
                            findingsByFile.put(relPath, fileFindings);
                        }
                    } catch (Exception ignored) {
                    }
                });
        } catch (Exception e) {
            findings.add(new VulnFinding(null, "MEDIUM", "扫描错误", dirPath,
                "扫描目录时出错: " + e.getMessage(), "请确认目录权限"));
            return findings;
        }

        // Phase 2: LLM verification per file's findings
        for (Map.Entry<String, List<VulnFinding>> entry : findingsByFile.entrySet()) {
            String relPath = entry.getKey();
            List<VulnFinding> candidates = entry.getValue();
            String codeContext = fileContents.getOrDefault(relPath, "");

            if (llm != null) {
                List<VulnFinding> verified = llm.verify(candidates, codeContext);
                if (verified != null) {
                    for (VulnFinding v : verified) {
                        if (v != null) {
                            findings.add(v);
                        }
                    }
                }
            } else {
                findings.addAll(candidates);
            }
        }

        return findings;
    }

    /**
     * Phase 1: run all regex rules against file content.
     * Returns candidate findings before LLM verification.
     */
    private static List<VulnFinding> scanFileWithRules(String content, String relPath) {
        List<VulnFinding> findings = new ArrayList<>();
        for (Rule rule : CODE_RULES) {
            for (Pattern p : rule.patterns) {
                Matcher m = p.matcher(content);
                if (m.find()) {
                    String matched = truncate(m.group(), 80);
                    String description = rule.defaultDescription != null
                        ? rule.defaultDescription
                        : "发现潜在的" + rule.type + "漏洞: " + matched;
                    findings.add(new VulnFinding(null, rule.severity, rule.type, relPath,
                        description, rule.defaultSuggestion));
                    break; // one finding per rule per file
                }
            }
        }
        return findings;
    }

    private static boolean isSourceFile(Path p) {
        String n = p.getFileName().toString().toLowerCase();
        return n.endsWith(".java") || n.endsWith(".kt") || n.endsWith(".py")
            || n.endsWith(".js") || n.endsWith(".ts") || n.endsWith(".tsx")
            || n.endsWith(".jsx") || n.endsWith(".go") || n.endsWith(".rs")
            || n.endsWith(".php") || n.endsWith(".rb") || n.endsWith(".cs")
            || n.endsWith(".c") || n.endsWith(".cpp") || n.endsWith(".h")
            || n.endsWith(".swift") || n.endsWith(".xml") || n.endsWith(".yml")
            || n.endsWith(".yaml") || n.endsWith(".properties")
            || n.endsWith(".conf") || n.endsWith(".cfg") || n.endsWith(".ini")
            || n.endsWith(".env") || n.endsWith(".sql") || n.endsWith(".sh")
            || n.endsWith(".bat") || n.endsWith(".ps1") || n.endsWith(".vue")
            || n.endsWith(".html") || n.endsWith(".jsp") || n.endsWith(".asp");
    }

    // ---- Utilities ----

    private static String truncate(String s, int maxLen) {
        if (s == null) return "";
        return s.length() <= maxLen ? s : s.substring(0, maxLen) + "...";
    }

    // ---- Agent integration ----

    @Override
    public VulnScanJob scanUrlWithAgent(String targetUrl, Long userId, Long deptId,
                                         String modelType, Long modelId,
                                         List<CredentialState> agentCredentials) {
        VulnScanJob job = createJob("url", targetUrl, userId, deptId, modelType, modelId);
        jobMapper.insertVulnScanJob(job);

        VulnLlmVerifier llm = buildVerifier(deptId, userId);
        List<VulnFinding> findings = doUrlScan(targetUrl);
        List<String> messages = new ArrayList<>();

        CompletableFuture<List<VulnFinding>> agentFuture = launchAgentIfConfigured(
            targetUrl, llm, agentCredentials, messages);

        try {
            List<VulnFinding> agentFindings = agentFuture.get(5, TimeUnit.MINUTES);
            if (agentFindings != null && !agentFindings.isEmpty()) {
                findings.addAll(agentFindings);
            }
        } catch (Exception e) {
            log.warn("Agent scan incomplete: {}", e.getMessage());
        }

        saveFindings(job.getId(), findings);
        updateJobCounts(job, findings);
        job.setFindings(findings);
        return job;
    }

    private CompletableFuture<List<VulnFinding>> launchAgentIfConfigured(
            String targetUrl, VulnLlmVerifier llm,
            List<CredentialState> agentCredentials,
            List<String> progressMessages) {
        if (llm == null) {
            log.info("Agent skipped: no LLM configured");
            return CompletableFuture.completedFuture(List.of());
        }

        PenTestAgent agent = new PenTestAgent(
            targetUrl,
            llm.getBaseUrl(),
            llm.getModel(),
            llm.getApiKey(),
            50,
            Duration.ofMinutes(55),
            msg -> {
                log.info("[Agent] {}", msg);
                if (progressMessages != null) progressMessages.add(msg);
            }
        );

        if (agentCredentials != null && !agentCredentials.isEmpty()) {
            agent.loadCredentials(agentCredentials);
        }

        return CompletableFuture.supplyAsync(() -> agent.run());
    }
}
