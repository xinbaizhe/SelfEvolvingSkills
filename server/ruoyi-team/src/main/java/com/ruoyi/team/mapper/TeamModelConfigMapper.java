package com.ruoyi.team.mapper;

import java.util.List;
import org.apache.ibatis.annotations.Param;
import com.ruoyi.team.domain.TeamModelConfig;

public interface TeamModelConfigMapper {
    TeamModelConfig selectTeamModelConfigById(Long id);
    List<TeamModelConfig> selectTeamModelConfigList(TeamModelConfig config);
    List<TeamModelConfig> selectAvailableModelConfigs(@Param("deptId") Long deptId, @Param("userId") Long userId, @Param("name") String name);
    List<TeamModelConfig> selectMyPersonalModelConfigs(@Param("createdBy") Long createdBy);
    int insertTeamModelConfig(TeamModelConfig config);
    int updateTeamModelConfig(TeamModelConfig config);
    int deleteTeamModelConfigById(Long id);
    int deleteTeamModelConfigByIds(Long[] ids);
    int deletePersonalModelConfigById(@Param("id") Long id, @Param("createdBy") Long createdBy);
}
