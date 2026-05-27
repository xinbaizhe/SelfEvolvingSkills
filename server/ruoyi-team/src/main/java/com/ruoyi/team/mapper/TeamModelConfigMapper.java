package com.ruoyi.team.mapper;

import java.util.List;
import com.ruoyi.team.domain.TeamModelConfig;

public interface TeamModelConfigMapper {
    TeamModelConfig selectTeamModelConfigById(Long id);
    List<TeamModelConfig> selectTeamModelConfigList(TeamModelConfig config);
    int insertTeamModelConfig(TeamModelConfig config);
    int updateTeamModelConfig(TeamModelConfig config);
    int deleteTeamModelConfigById(Long id);
    int deleteTeamModelConfigByIds(Long[] ids);
}
