package com.ruoyi.team.service.impl;

import java.util.List;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;
import com.ruoyi.common.annotation.DataScope;
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
    @DataScope(deptAlias = "c", deptField = "dept_id")
    public List<TeamModelConfig> selectTeamModelConfigList(TeamModelConfig config) {
        return modelConfigMapper.selectTeamModelConfigList(config);
    }

    @Override
    public List<TeamModelConfig> selectActiveModelConfigsByDeptId(Long deptId) {
        return modelConfigMapper.selectAvailableModelConfigs(deptId, null, null);
    }

    @Override
    public List<TeamModelConfig> selectAvailableModelConfigs(Long deptId, Long userId, String name) {
        return modelConfigMapper.selectAvailableModelConfigs(deptId, userId, name);
    }

    @Override
    public List<TeamModelConfig> selectMyPersonalModelConfigs(Long createdBy) {
        return modelConfigMapper.selectMyPersonalModelConfigs(createdBy);
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

    @Override
    public int deletePersonalModelConfigById(Long id, Long createdBy) {
        return modelConfigMapper.deletePersonalModelConfigById(id, createdBy);
    }
}
