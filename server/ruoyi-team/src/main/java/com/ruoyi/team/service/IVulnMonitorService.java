package com.ruoyi.team.service;

import java.util.List;
import java.util.Map;
import com.ruoyi.team.domain.DepMonitor;
import com.ruoyi.team.domain.DepFinding;

public interface IVulnMonitorService {
    List<DepMonitor> uploadManifest(String name, List<Map<String, String>> files, Long userId, Long deptId);
    List<DepMonitor> getSnapshots(Long userId);
    List<DepMonitor> getDependencies(Long userId, String name);
    List<DepFinding> getFindings(Long depId);
    List<DepFinding> refreshSnapshot(Long snapshotId, Long userId);
    void deleteSnapshot(Long snapshotId, Long userId);
}
