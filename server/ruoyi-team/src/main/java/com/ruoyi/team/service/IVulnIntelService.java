package com.ruoyi.team.service;

import java.util.List;
import com.ruoyi.team.domain.VulnIntel;

public interface IVulnIntelService {
    List<VulnIntel> list(String vulnType, String severity, String keyword, String startDate, String endDate);
    int syncPublicIntel();
}
