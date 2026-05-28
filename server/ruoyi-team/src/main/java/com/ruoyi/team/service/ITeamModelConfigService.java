package com.ruoyi.team.service;

import java.util.List;
import com.ruoyi.team.domain.TeamModelConfig;

public interface ITeamModelConfigService {
    TeamModelConfig selectTeamModelConfigById(Long id);
    List<TeamModelConfig> selectTeamModelConfigList(TeamModelConfig config);
    List<TeamModelConfig> selectActiveModelConfigsByDeptId(Long deptId);
    List<TeamModelConfig> selectAvailableModelConfigs(Long deptId, Long userId, String name);
    List<TeamModelConfig> selectMyPersonalModelConfigs(Long createdBy);
    List<TeamModelConfig> selectAllActiveModelConfigs();
    int insertTeamModelConfig(TeamModelConfig config);
    int updateTeamModelConfig(TeamModelConfig config);
    int deleteTeamModelConfigByIds(Long[] ids);
    int deletePersonalModelConfigById(Long id, Long createdBy);
}
