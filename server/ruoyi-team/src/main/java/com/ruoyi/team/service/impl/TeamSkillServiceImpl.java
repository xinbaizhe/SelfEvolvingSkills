package com.ruoyi.team.service.impl;

import java.util.List;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;
import com.ruoyi.team.domain.TeamSkill;
import com.ruoyi.team.mapper.TeamSkillMapper;
import com.ruoyi.team.service.ITeamSkillService;

@Service
public class TeamSkillServiceImpl implements ITeamSkillService {

    @Autowired
    private TeamSkillMapper teamSkillMapper;

    @Override
    public TeamSkill selectTeamSkillById(Long id) {
        return teamSkillMapper.selectTeamSkillById(id);
    }

    @Override
    public List<TeamSkill> selectTeamSkillList(TeamSkill skill) {
        return teamSkillMapper.selectTeamSkillList(skill);
    }

    @Override
    public int insertTeamSkill(TeamSkill skill) {
        return teamSkillMapper.insertTeamSkill(skill);
    }

    @Override
    public int updateTeamSkill(TeamSkill skill) {
        return teamSkillMapper.updateTeamSkill(skill);
    }

    @Override
    public int deleteTeamSkillByIds(Long[] ids) {
        return teamSkillMapper.deleteTeamSkillByIds(ids);
    }

    @Override
    public int incrementUsageCount(Long id) {
        TeamSkill skill = teamSkillMapper.selectTeamSkillById(id);
        if (skill != null) {
            skill.setUsageCount(skill.getUsageCount() != null ? skill.getUsageCount() + 1 : 1);
            return teamSkillMapper.updateTeamSkill(skill);
        }
        return 0;
    }
}
