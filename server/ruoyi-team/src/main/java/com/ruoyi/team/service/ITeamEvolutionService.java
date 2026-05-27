package com.ruoyi.team.service;

import java.util.List;
import com.ruoyi.team.domain.TeamEvolution;

public interface ITeamEvolutionService {
    TeamEvolution selectTeamEvolutionById(Long id);
    List<TeamEvolution> selectTeamEvolutionList(TeamEvolution evolution);
    List<TeamEvolution> selectTeamEvolutionBySkillId(Long skillId);
    int insertTeamEvolution(TeamEvolution evolution);
    int updateTeamEvolution(TeamEvolution evolution);
    int approveTeamEvolution(TeamEvolution evolution);
    void applyEvolutionToSkill(TeamEvolution evolution);
    int deleteTeamEvolutionById(Long id);
}
