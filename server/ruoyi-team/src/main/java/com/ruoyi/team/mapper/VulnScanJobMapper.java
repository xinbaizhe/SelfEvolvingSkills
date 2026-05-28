package com.ruoyi.team.mapper;

import java.util.List;
import com.ruoyi.team.domain.VulnScanJob;

public interface VulnScanJobMapper {
    VulnScanJob selectVulnScanJobById(Long id);
    List<VulnScanJob> selectVulnScanJobList(VulnScanJob job);
    int insertVulnScanJob(VulnScanJob job);
    int updateVulnScanJob(VulnScanJob job);
    int deleteVulnScanJobById(Long id);
}
