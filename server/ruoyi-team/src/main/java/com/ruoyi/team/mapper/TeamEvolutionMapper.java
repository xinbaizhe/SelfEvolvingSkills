package com.ruoyi.team.mapper;

import java.util.List;
import com.ruoyi.team.domain.TeamEvolution;

public interface TeamEvolutionMapper {
    TeamEvolution selectTeamEvolutionById(Long id);
    List<TeamEvolution> selectTeamEvolutionList(TeamEvolution evolution);
    List<TeamEvolution> selectTeamEvolutionBySkillId(Long skillId);
    int insertTeamEvolution(TeamEvolution evolution);
    int updateTeamEvolution(TeamEvolution evolution);
    int approveTeamEvolution(TeamEvolution evolution);
    int deleteTeamEvolutionById(Long id);
}
