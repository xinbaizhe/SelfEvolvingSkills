package com.ruoyi.team.service;

import java.util.List;
import com.ruoyi.team.domain.VulnScanJob;

public interface IVulnScanService {
    VulnScanJob scanUrl(String url, Long userId, Long deptId, String modelType, Long modelId);
    VulnScanJob scanCode(String dirPath, Long userId, Long deptId, String modelType, Long modelId);
    VulnScanJob getJobWithFindings(Long jobId);
    List<VulnScanJob> getHistory(Long userId);
}
