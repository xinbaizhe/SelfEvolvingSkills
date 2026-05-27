package com.ruoyi.team.service.impl;

import java.util.List;
import java.util.stream.Collectors;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;
import com.ruoyi.team.domain.TeamModelConfig;
import com.ruoyi.team.mapper.TeamModelConfigMapper;
import com.ruoyi.team.service.ITeamModelConfigService;

@Service
public class TeamModelConfigServiceImpl implements ITeamModelConfigService {

    @Autowired
    private TeamModelConfigMapper modelConfigMapper;

    @Override
    public TeamModelConfig selectTeamModelConfigById(Long id) {
        return modelConfigMapper.selectTeamModelConfigById(id);
    }

    @Override
    public List<TeamModelConfig> selectTeamModelConfigList(TeamModelConfig config) {
        return modelConfigMapper.selectTeamModelConfigList(config);
    }

    @Override
    public List<TeamModelConfig> selectActiveModelConfigsByDeptId(Long deptId) {
        TeamModelConfig query = new TeamModelConfig();
        query.setDeptId(deptId);
        query.setIsActive(1);
        return modelConfigMapper.selectTeamModelConfigList(query);
    }

    @Override
    public List<TeamModelConfig> selectAllActiveModelConfigs() {
        TeamModelConfig query = new TeamModelConfig();
        query.setIsActive(1);
        return modelConfigMapper.selectTeamModelConfigList(query);
    }

    @Override
    public int insertTeamModelConfig(TeamModelConfig config) {
        return modelConfigMapper.insertTeamModelConfig(config);
    }

    @Override
    public int updateTeamModelConfig(TeamModelConfig config) {
        return modelConfigMapper.updateTeamModelConfig(config);
    }

    @Override
    public int deleteTeamModelConfigByIds(Long[] ids) {
        return modelConfigMapper.deleteTeamModelConfigByIds(ids);
    }
}
