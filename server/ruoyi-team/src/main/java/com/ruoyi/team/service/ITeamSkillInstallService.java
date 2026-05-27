package com.ruoyi.team.service;

import java.util.List;
import com.ruoyi.team.domain.TeamSkillInstall;

public interface ITeamSkillInstallService {
    TeamSkillInstall selectTeamSkillInstallById(Long id);
    List<TeamSkillInstall> selectTeamSkillInstallList(TeamSkillInstall install);
    int insertTeamSkillInstall(TeamSkillInstall install);
    int deleteTeamSkillInstallById(Long id);
    boolean checkInstallUnique(TeamSkillInstall install);
}
