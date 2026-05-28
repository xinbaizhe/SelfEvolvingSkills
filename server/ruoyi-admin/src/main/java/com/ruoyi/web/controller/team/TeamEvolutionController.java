package com.ruoyi.web.controller.team;

import java.time.LocalDateTime;
import java.util.List;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;
import com.ruoyi.common.core.controller.BaseController;
import com.ruoyi.common.core.domain.AjaxResult;
import com.ruoyi.common.core.page.TableDataInfo;
import com.ruoyi.common.utils.SecurityUtils;
import com.ruoyi.team.domain.TeamEvolution;
import com.ruoyi.team.service.ITeamEvolutionService;

@RestController
@RequestMapping("/api/team/evolutions")
@PreAuthorize("isAuthenticated()")
public class TeamEvolutionController extends BaseController {

    @Autowired
    private ITeamEvolutionService evolutionService;

    @GetMapping("/list")
    public TableDataInfo list(TeamEvolution evolution) {
        startPage();
        List<TeamEvolution> list = evolutionService.selectTeamEvolutionList(evolution);
        return getDataTable(list);
    }

    @GetMapping("/skill/{skillId}")
    public AjaxResult listBySkill(@PathVariable Long skillId) {
        List<TeamEvolution> list = evolutionService.selectTeamEvolutionBySkillId(skillId);
        return success(list);
    }

    @GetMapping("/{id}")
    public AjaxResult getInfo(@PathVariable Long id) {
        return success(evolutionService.selectTeamEvolutionById(id));
    }

    @PostMapping
    public AjaxResult submit(@RequestBody TeamEvolution evolution) {
        evolution.setProposerId(SecurityUtils.getUserId());
        evolution.setStatus("pending");
        evolution.setCreatedAt(LocalDateTime.now());
        return toAjax(evolutionService.insertTeamEvolution(evolution));
    }

    @PostMapping("/{id}/approve")
    public AjaxResult approve(@PathVariable Long id, @RequestBody TeamEvolution body) {
        TeamEvolution evolution = new TeamEvolution();
        evolution.setId(id);
        evolution.setStatus(body.getStatus());
        evolution.setReviewerId(SecurityUtils.getUserId());
        evolution.setReviewComment(body.getReviewComment());
        evolution.setReviewedAt(LocalDateTime.now());

        int rows = evolutionService.approveTeamEvolution(evolution);
        if (rows > 0 && "approved".equals(body.getStatus())) {
            TeamEvolution approved = evolutionService.selectTeamEvolutionById(id);
            if (approved != null) {
                evolutionService.applyEvolutionToSkill(approved);
            }
        }
        return toAjax(rows);
    }
}
