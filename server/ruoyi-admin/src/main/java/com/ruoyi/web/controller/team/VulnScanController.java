package com.ruoyi.web.controller.team;

import java.util.HashMap;
import java.util.List;
import java.util.Map;

import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import com.ruoyi.common.annotation.Anonymous;
import com.ruoyi.common.core.controller.BaseController;
import com.ruoyi.common.core.domain.AjaxResult;
import com.ruoyi.common.core.domain.model.LoginUser;
import com.ruoyi.common.utils.SecurityUtils;
import com.ruoyi.team.domain.VulnScanJob;
import com.ruoyi.team.service.IVulnScanService;

@RestController
@RequestMapping("/api/vuln")
public class VulnScanController extends BaseController {

    @Autowired
    private IVulnScanService vulnScanService;

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
            loginUser.getUser().getDeptId()
        );
        return success(job);
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
            loginUser.getUser().getDeptId()
        );
        return success(job);
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
    public AjaxResult getHistory() {
        LoginUser loginUser = SecurityUtils.getLoginUser();
        List<VulnScanJob> history = vulnScanService.getHistory(
            loginUser.getUser().getUserId()
        );
        return success(history);
    }
}
