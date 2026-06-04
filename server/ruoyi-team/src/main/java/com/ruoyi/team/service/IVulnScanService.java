package com.ruoyi.team.service;

import java.util.List;
import java.util.Map;
import java.util.function.Consumer;

import com.ruoyi.team.domain.CredentialState;
import com.ruoyi.team.domain.VulnScanJob;

public interface IVulnScanService {
    VulnScanJob scanUrl(String url, Long userId, Long deptId, String modelType, Long modelId,
                        Map<String, String> requestHeaders, String scanProfile, String customPaths,
                        Integer maxDepth, Integer maxPages, Boolean portScanEnabled, String portSpec);
    VulnScanJob scanCode(String dirPath, Long userId, Long deptId, String modelType, Long modelId);
    VulnScanJob scanUrlStream(String url, Long userId, Long deptId, String modelType, Long modelId,
                              Map<String, String> requestHeaders, String scanProfile, String customPaths,
                              Integer maxDepth, Integer maxPages, Boolean portScanEnabled, String portSpec,
                              Consumer<String> progressCallback);
    VulnScanJob getJobWithFindings(Long jobId);
    List<VulnScanJob> getHistory(Long userId);

    VulnScanJob scanUrlWithAgent(String targetUrl, Long userId, Long deptId,
                                  String modelType, Long modelId,
                                  List<CredentialState> agentCredentials);

    VulnScanJob scanUrlWithAgentStream(String targetUrl, Long userId, Long deptId,
                                        String modelType, Long modelId,
                                        List<CredentialState> agentCredentials,
                                        Consumer<String> progressCallback);
}
