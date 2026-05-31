package com.ruoyi.team.mapper;

import java.util.List;
import org.apache.ibatis.annotations.Param;
import com.ruoyi.team.domain.DepMonitor;

public interface DepMonitorMapper {
    List<DepMonitor> selectByUserId(Long userId);
    List<DepMonitor> selectByUserIdAndName(@Param("userId") Long userId, @Param("name") String name);
    int insertDepMonitor(DepMonitor dep);
    int batchInsertDepMonitor(List<DepMonitor> deps);
    int updateDepMonitorCounts(DepMonitor dep);
    int updateDepMonitorCheckedAt(Long id);
    int deleteDepMonitorById(Long id);
    int deleteDepMonitorByUserIdAndName(@Param("userId") Long userId, @Param("name") String name);
}
