package com.ruoyi.web.controller.team;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;

import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;
import org.springframework.web.servlet.mvc.method.annotation.SseEmitter;

import com.ruoyi.common.annotation.Anonymous;
import com.ruoyi.common.core.controller.BaseController;
import com.ruoyi.common.core.domain.AjaxResult;
import com.ruoyi.common.core.domain.model.LoginUser;
import com.ruoyi.common.core.page.TableDataInfo;
import com.ruoyi.common.utils.SecurityUtils;
import com.ruoyi.team.domain.CredentialState;
import com.ruoyi.team.domain.DepFinding;
import com.ruoyi.team.domain.DepMonitor;
import com.ruoyi.team.domain.VulnIntel;
import com.ruoyi.team.domain.VulnScanJob;
import com.ruoyi.team.service.IVulnIntelService;
import com.ruoyi.team.service.IVulnMonitorService;
import com.ruoyi.team.service.IVulnScanService;

@RestController
@RequestMapping("/api/vuln")
public class VulnScanController extends BaseController {

    @Autowired
    private IVulnScanService vulnScanService;

    @Autowired
    private IVulnIntelService vulnIntelService;

    @Autowired
    private IVulnMonitorService vulnMonitorService;

    @Anonymous
    @GetMapping("/ping")
    public AjaxResult ping() {
        Map<String, Object> result = new HashMap<>();
        result.put("status", "ok");
        result.put("version", "2.0.1");
        return success(result);
    }

    @PostMapping("/scan-url")
    public AjaxResult scanUrl(@RequestBody Map<String, String> body) {
        String url = body.get("url");
        if (url == null || url.isBlank()) {
            return error("URL不能为空");
        }

        LoginUser loginUser = SecurityUtils.getLoginUser();
        VulnScanJob job = vulnScanService.scanUrl(
            url.trim(),
            loginUser.getUser().getUserId(),
            loginUser.getUser().getDeptId(),
            body.get("modelType"),
            parseLong(body.get("modelId")),
            buildScanHeaders(body),
            body.get("scanProfile"),
            body.get("customPaths"),
            parseInteger(body.get("maxDepth")),
            parseInteger(body.get("maxPages")),
            parseBoolean(body.get("portScanEnabled")),
            body.get("portSpec")
        );
        return success(job);
    }

    @SuppressWarnings("unchecked")
    @PostMapping("/scan-url/agent")
    public AjaxResult scanUrlWithAgent(@RequestBody Map<String, Object> body) {
        String url = (String) body.get("url");
        if (url == null || url.isBlank()) {
            return error("URL不能为空");
        }

        List<CredentialState> creds = new ArrayList<>();
        Object credsObj = body.get("agentCredentials");
        if (credsObj instanceof List<?> list) {
            for (Object item : list) {
                if (item instanceof Map) {
                    @SuppressWarnings("unchecked")
                    Map<String, Object> m = (Map<String, Object>) item;
                    creds.add(new CredentialState(
                        String.valueOf(m.getOrDefault("credId", "")),
                        String.valueOf(m.getOrDefault("role", "unknown")),
                        String.valueOf(m.getOrDefault("username", "")),
                        String.valueOf(m.getOrDefault("permissions", "")),
                        !"false".equals(String.valueOf(m.get("sessionValid"))),
                        String.valueOf(m.getOrDefault("cookie", "")),
                        String.valueOf(m.getOrDefault("authorization", ""))
                    ));
                }
            }
        }

        LoginUser loginUser = SecurityUtils.getLoginUser();
        VulnScanJob job = vulnScanService.scanUrlWithAgent(
            url.trim(),
            loginUser.getUser().getUserId(),
            loginUser.getUser().getDeptId(),
            (String) body.get("modelType"),
            parseLong((String) body.get("modelId")),
            creds
        );
        return success(job);
    }

    @PostMapping(value = "/scan-url/stream", produces = MediaType.TEXT_EVENT_STREAM_VALUE)
    public SseEmitter scanUrlStream(@RequestBody Map<String, String> body) {
        String url = body.get("url");
        if (url == null || url.isBlank()) {
            throw new IllegalArgumentException("URL不能为空");
        }

        LoginUser loginUser = SecurityUtils.getLoginUser();
        SseEmitter emitter = new SseEmitter(300_000L); // 5 minute timeout

        CompletableFuture.runAsync(() -> {
            try {
                VulnScanJob job = vulnScanService.scanUrlStream(
                    url.trim(),
                    loginUser.getUser().getUserId(),
                    loginUser.getUser().getDeptId(),
                    body.get("modelType"),
                    parseLong(body.get("modelId")),
                    buildScanHeaders(body),
                    body.get("scanProfile"),
                    body.get("customPaths"),
                    parseInteger(body.get("maxDepth")),
                    parseInteger(body.get("maxPages")),
                    parseBoolean(body.get("portScanEnabled")),
                    body.get("portSpec"),
                    msg -> {
                        try {
                            emitter.send(SseEmitter.event().name("progress").data(msg));
                        } catch (Exception ignored) {
                            // client disconnected
                        }
                    }
                );
                emitter.send(SseEmitter.event().name("complete").data(job));
                emitter.complete();
            } catch (Exception e) {
                try {
                    emitter.send(SseEmitter.event().name("error").data(e.getMessage()));
                    emitter.completeWithError(e);
                } catch (Exception ignored) {
                    // client disconnected
                }
            }
        });

        return emitter;
    }

    @PostMapping("/scan-code")
    public AjaxResult scanCode(@RequestBody Map<String, String> body) {
        String path = body.get("path");
        if (path == null || path.isBlank()) {
            return error("目录路径不能为空");
        }

        LoginUser loginUser = SecurityUtils.getLoginUser();
        VulnScanJob job = vulnScanService.scanCode(
            path.trim(),
            loginUser.getUser().getUserId(),
            loginUser.getUser().getDeptId(),
            body.get("modelType"),
            parseLong(body.get("modelId"))
        );
        return success(job);
    }

    @PostMapping("/intel/sync")
    public AjaxResult syncIntel() {
        int count = vulnIntelService.syncPublicIntel();
        Map<String, Object> result = new HashMap<>();
        result.put("count", count);
        return success(result);
    }

    @GetMapping("/intel/list")
    public AjaxResult intelList(String vulnType, String severity, String keyword, String startDate, String endDate) {
        List<VulnIntel> list = vulnIntelService.list(vulnType, severity, keyword, startDate, endDate);
        return success(list);
    }

    @PostMapping("/monitor/upload")
    @SuppressWarnings("unchecked")
    public AjaxResult uploadManifest(@RequestBody Map<String, Object> body) {
        String name = (String) body.get("name");
        List<Map<String, String>> files = (List<Map<String, String>>) body.get("files");
        if (name == null || name.isBlank() || files == null || files.isEmpty()) {
            return error("name 和 files 不能为空");
        }
        LoginUser loginUser = SecurityUtils.getLoginUser();
        List<DepMonitor> result = vulnMonitorService.uploadManifest(
            name.trim(), files,
            loginUser.getUser().getUserId(),
            loginUser.getUser().getDeptId()
        );
        Map<String, Object> data = new HashMap<>();
        data.put("snapshot", name);
        data.put("deps", result);
        return success(data);
    }

    @GetMapping("/monitor/snapshots")
    public AjaxResult getSnapshots() {
        LoginUser loginUser = SecurityUtils.getLoginUser();
        List<DepMonitor> snapshots = vulnMonitorService.getSnapshots(loginUser.getUser().getUserId());
        return success(snapshots);
    }

    @GetMapping("/monitor/{id}/deps")
    public AjaxResult getMonitorDeps(@PathVariable Long id, @RequestParam String name) {
        LoginUser loginUser = SecurityUtils.getLoginUser();
        return success(vulnMonitorService.getDependencies(loginUser.getUser().getUserId(), name));
    }

    @GetMapping("/monitor/dep/{depId}/findings")
    public AjaxResult getDepFindings(@PathVariable Long depId) {
        return success(vulnMonitorService.getFindings(depId));
    }

    @PostMapping("/monitor/{id}/refresh")
    public AjaxResult refreshMonitor(@PathVariable Long id) {
        LoginUser loginUser = SecurityUtils.getLoginUser();
        List<DepFinding> findings = vulnMonitorService.refreshSnapshot(id, loginUser.getUser().getUserId());
        return success(findings);
    }

    @DeleteMapping("/monitor/{id}")
    public AjaxResult deleteMonitor(@PathVariable Long id) {
        LoginUser loginUser = SecurityUtils.getLoginUser();
        vulnMonitorService.deleteSnapshot(id, loginUser.getUser().getUserId());
        return success();
    }

    @GetMapping("/scan/{jobId}")
    public AjaxResult getScanResult(@PathVariable Long jobId) {
        VulnScanJob job = vulnScanService.getJobWithFindings(jobId);
        if (job == null) {
            return error("扫描任务不存在");
        }
        return success(job);
    }

    @GetMapping("/history")
    public TableDataInfo getHistory() {
        LoginUser loginUser = SecurityUtils.getLoginUser();
        startPage();
        List<VulnScanJob> history = vulnScanService.getHistory(
            loginUser.getUser().getUserId()
        );
        return getDataTable(history);
    }

    private Long parseLong(String value) {
        if (value == null || value.isBlank()) return null;
        try {
            return Long.parseLong(value);
        } catch (Exception ignored) {
            return null;
        }
    }

    private Integer parseInteger(String value) {
        if (value == null || value.isBlank()) return null;
        try {
            return Integer.parseInt(value);
        } catch (Exception ignored) {
            return null;
        }
    }

    private Boolean parseBoolean(String value) {
        if (value == null || value.isBlank()) return null;
        return "true".equalsIgnoreCase(value) || "1".equals(value) || "yes".equalsIgnoreCase(value);
    }

    private Map<String, String> buildScanHeaders(Map<String, String> body) {
        Map<String, String> headers = new HashMap<>();
        putHeader(headers, "Cookie", body.get("cookie"));
        putHeader(headers, "Authorization", body.get("authorization"));
        String customHeaders = body.get("headers");
        if (customHeaders != null && !customHeaders.isBlank()) {
            String[] lines = customHeaders.split("\\r?\\n");
            for (String line : lines) {
                int idx = line.indexOf(':');
                if (idx > 0) {
                    putHeader(headers, line.substring(0, idx).trim(), line.substring(idx + 1).trim());
                }
            }
        }
        return headers;
    }

    private void putHeader(Map<String, String> headers, String name, String value) {
        if (name == null || name.isBlank() || value == null || value.isBlank()) return;
        if ("Host".equalsIgnoreCase(name) || "Content-Length".equalsIgnoreCase(name)) return;
        headers.put(name, value);
    }
}
