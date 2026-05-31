package com.ruoyi.team.mapper;

import java.util.List;
import com.ruoyi.team.domain.DepFinding;

public interface DepFindingMapper {
    List<DepFinding> selectByDepId(Long depId);
    int batchInsertDepFindings(List<DepFinding> findings);
    int deleteByDepId(Long depId);
}
