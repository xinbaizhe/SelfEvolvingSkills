package com.ruoyi.team.mapper;

import java.util.List;
import com.ruoyi.team.domain.TeamSkill;

public interface TeamSkillMapper {
    TeamSkill selectTeamSkillById(Long id);
    List<TeamSkill> selectTeamSkillList(TeamSkill skill);
    int insertTeamSkill(TeamSkill skill);
    int updateTeamSkill(TeamSkill skill);
    int deleteTeamSkillById(Long id);
    int deleteTeamSkillByIds(Long[] ids);
}
