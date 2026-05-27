package com.ruoyi.team.mapper;

import java.util.List;
import com.ruoyi.team.domain.TeamSkillInstall;

public interface TeamSkillInstallMapper {
    TeamSkillInstall selectTeamSkillInstallById(Long id);
    List<TeamSkillInstall> selectTeamSkillInstallList(TeamSkillInstall install);
    int insertTeamSkillInstall(TeamSkillInstall install);
    int deleteTeamSkillInstallById(Long id);
    int deleteTeamSkillInstallBySkillId(Long skillId);
    TeamSkillInstall checkInstallUnique(TeamSkillInstall install);
}
