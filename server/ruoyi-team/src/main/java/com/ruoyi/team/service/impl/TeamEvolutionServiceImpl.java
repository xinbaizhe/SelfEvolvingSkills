package com.ruoyi.team.service.impl;

import java.util.List;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;
import com.ruoyi.team.domain.TeamEvolution;
import com.ruoyi.team.domain.TeamSkill;
import com.ruoyi.team.mapper.TeamEvolutionMapper;
import com.ruoyi.team.mapper.TeamSkillMapper;
import com.ruoyi.team.service.ITeamEvolutionService;

@Service
public class TeamEvolutionServiceImpl implements ITeamEvolutionService {

    @Autowired
    private TeamEvolutionMapper evolutionMapper;

    @Autowired
    private TeamSkillMapper skillMapper;

    @Override
    public TeamEvolution selectTeamEvolutionById(Long id) {
        return evolutionMapper.selectTeamEvolutionById(id);
    }

    @Override
    public List<TeamEvolution> selectTeamEvolutionList(TeamEvolution evolution) {
        return evolutionMapper.selectTeamEvolutionList(evolution);
    }

    @Override
    public List<TeamEvolution> selectTeamEvolutionBySkillId(Long skillId) {
        return evolutionMapper.selectTeamEvolutionBySkillId(skillId);
    }

    @Override
    public int insertTeamEvolution(TeamEvolution evolution) {
        return evolutionMapper.insertTeamEvolution(evolution);
    }

    @Override
    public int updateTeamEvolution(TeamEvolution evolution) {
        return evolutionMapper.updateTeamEvolution(evolution);
    }

    @Override
    public int approveTeamEvolution(TeamEvolution evolution) {
        return evolutionMapper.approveTeamEvolution(evolution);
    }

    @Override
    @Transactional
    public void applyEvolutionToSkill(TeamEvolution evolution) {
        TeamSkill skill = skillMapper.selectTeamSkillById(evolution.getSkillId());
        if (skill == null) return;

        skill.setBodyMd(evolution.getProposedChange());
        skill.setVersion(skill.getVersion() != null ? skill.getVersion() + 1 : 2);
        skillMapper.updateTeamSkill(skill);
    }

    @Override
    public int deleteTeamEvolutionById(Long id) {
        return evolutionMapper.deleteTeamEvolutionById(id);
    }
}
