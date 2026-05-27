package com.ruoyi.team.service;

import java.util.List;
import com.ruoyi.team.domain.TeamSkill;

public interface ITeamSkillService {
    TeamSkill selectTeamSkillById(Long id);
    List<TeamSkill> selectTeamSkillList(TeamSkill skill);
    int insertTeamSkill(TeamSkill skill);
    int updateTeamSkill(TeamSkill skill);
    int deleteTeamSkillByIds(Long[] ids);
    int incrementUsageCount(Long id);
}
