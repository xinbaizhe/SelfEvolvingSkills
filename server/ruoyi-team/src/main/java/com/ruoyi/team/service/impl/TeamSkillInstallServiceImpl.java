package com.ruoyi.team.service.impl;

import java.util.List;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;
import com.ruoyi.team.domain.TeamSkillInstall;
import com.ruoyi.team.mapper.TeamSkillInstallMapper;
import com.ruoyi.team.service.ITeamSkillInstallService;

@Service
public class TeamSkillInstallServiceImpl implements ITeamSkillInstallService {

    @Autowired
    private TeamSkillInstallMapper installMapper;

    @Override
    public TeamSkillInstall selectTeamSkillInstallById(Long id) {
        return installMapper.selectTeamSkillInstallById(id);
    }

    @Override
    public List<TeamSkillInstall> selectTeamSkillInstallList(TeamSkillInstall install) {
        return installMapper.selectTeamSkillInstallList(install);
    }

    @Override
    public int insertTeamSkillInstall(TeamSkillInstall install) {
        return installMapper.insertTeamSkillInstall(install);
    }

    @Override
    public int deleteTeamSkillInstallById(Long id) {
        return installMapper.deleteTeamSkillInstallById(id);
    }

    @Override
    public boolean checkInstallUnique(TeamSkillInstall install) {
        TeamSkillInstall existing = installMapper.checkInstallUnique(install);
        return existing == null;
    }
}
