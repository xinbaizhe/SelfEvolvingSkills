package com.ruoyi.web.controller.team;

import java.util.List;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;
import com.ruoyi.common.core.controller.BaseController;
import com.ruoyi.common.core.domain.AjaxResult;
import com.ruoyi.common.core.page.TableDataInfo;
import com.ruoyi.common.utils.SecurityUtils;
import com.ruoyi.team.domain.TeamModelConfig;
import com.ruoyi.team.service.ITeamModelConfigService;

@RestController
@RequestMapping("/api/team/models")
public class TeamModelConfigController extends BaseController {

    @Autowired
    private ITeamModelConfigService modelConfigService;

    @GetMapping("/list")
    public TableDataInfo list(TeamModelConfig config) {
        startPage();
        List<TeamModelConfig> list = modelConfigService.selectTeamModelConfigList(config);
        return getDataTable(list);
    }

    @GetMapping("/available")
    public AjaxResult available() {
        Long deptId = SecurityUtils.getLoginUser().getUser().getDeptId();
        List<TeamModelConfig> list = modelConfigService.selectActiveModelConfigsByDeptId(deptId);
        return success(list);
    }

    @GetMapping("/{id}")
    public AjaxResult getInfo(@PathVariable Long id) {
        return success(modelConfigService.selectTeamModelConfigById(id));
    }

    @PostMapping
    public AjaxResult add(@RequestBody TeamModelConfig config) {
        config.setCreatedBy(SecurityUtils.getUserId());
        return toAjax(modelConfigService.insertTeamModelConfig(config));
    }

    @PutMapping
    public AjaxResult edit(@RequestBody TeamModelConfig config) {
        return toAjax(modelConfigService.updateTeamModelConfig(config));
    }

    @DeleteMapping("/{ids}")
    public AjaxResult remove(@PathVariable Long[] ids) {
        return toAjax(modelConfigService.deleteTeamModelConfigByIds(ids));
    }
}
