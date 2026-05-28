package com.ruoyi.team.mapper;

import java.util.List;
import com.ruoyi.team.domain.VulnFinding;

public interface VulnFindingMapper {
    List<VulnFinding> selectFindingsByJobId(Long jobId);
    int insertVulnFinding(VulnFinding finding);
    int batchInsertVulnFindings(List<VulnFinding> findings);
    int deleteFindingsByJobId(Long jobId);
}
