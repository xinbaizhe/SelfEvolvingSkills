package com.ruoyi.team.mapper;

import java.util.List;
import org.apache.ibatis.annotations.Param;
import com.ruoyi.team.domain.VulnIntel;

public interface VulnIntelMapper {
    List<VulnIntel> selectVulnIntelList(@Param("vulnType") String vulnType,
                                        @Param("severity") String severity,
                                        @Param("keyword") String keyword,
                                        @Param("startDate") String startDate,
                                        @Param("endDate") String endDate);
    int upsertVulnIntel(VulnIntel intel);
}
