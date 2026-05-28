package com.ruoyi.web.controller.team;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.security.access.prepost.PreAuthorize;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;
import com.ruoyi.common.core.controller.BaseController;
import com.ruoyi.common.core.domain.AjaxResult;
import com.ruoyi.common.core.domain.entity.SysUser;
import com.ruoyi.common.core.page.TableDataInfo;
import com.ruoyi.common.utils.SecurityUtils;
import com.ruoyi.common.utils.StringUtils;
import com.ruoyi.system.service.ISysUserService;
import com.ruoyi.team.domain.TeamModelConfig;
import com.ruoyi.team.service.ITeamModelConfigService;

@RestController
@RequestMapping("/api/team/models")
@PreAuthorize("isAuthenticated()")
public class TeamModelConfigController extends BaseController {

    @Autowired
    private ITeamModelConfigService modelConfigService;

    @Autowired
    private ISysUserService userService;

    @GetMapping("/list")
    @PreAuthorize("@ss.hasPermi('system:model:list')")
    public TableDataInfo list(TeamModelConfig config) {
        startPage();
        List<TeamModelConfig> list = modelConfigService.selectTeamModelConfigList(config);
        return getDataTable(list);
    }

    @GetMapping("/available")
    public AjaxResult available(@RequestParam(required = false) String name) {
        SysUser user = SecurityUtils.getLoginUser().getUser();
        Long deptId = user.getDeptId();
        List<TeamModelConfig> list = modelConfigService.selectAvailableModelConfigs(deptId, user.getUserId(), name);
        return success(list);
    }

    @GetMapping("/personal/my")
    public AjaxResult myPersonal() {
        return success(modelConfigService.selectMyPersonalModelConfigs(SecurityUtils.getUserId()));
    }

    @GetMapping("/users/search")
    public AjaxResult searchUsers(@RequestParam String username) {
        if (StringUtils.isBlank(username)) {
            return error("请输入用户名称");
        }
        SysUser query = new SysUser();
        query.setUserName(username);
        query.setStatus("0");
        List<Map<String, Object>> users = userService.selectUserList(query).stream()
            .limit(10)
            .map(user -> {
                Map<String, Object> item = new HashMap<>();
                item.put("userId", user.getUserId());
                item.put("userName", user.getUserName());
                item.put("nickName", user.getNickName());
                item.put("deptId", user.getDeptId());
                item.put("deptName", user.getDept() != null ? user.getDept().getDeptName() : "");
                return item;
            })
            .toList();
        return success(users);
    }

    @GetMapping("/{id}")
    public AjaxResult getInfo(@PathVariable Long id) {
        return success(modelConfigService.selectTeamModelConfigById(id));
    }

    @PostMapping
    @PreAuthorize("@ss.hasPermi('system:model:add')")
    public AjaxResult add(@RequestBody TeamModelConfig config) {
        if (config.getIsActive() == null) {
            config.setIsActive(1);
        }
        config.setCreatedBy(SecurityUtils.getUserId());
        return toAjax(modelConfigService.insertTeamModelConfig(config));
    }

    @PostMapping("/personal")
    public AjaxResult addPersonal(@RequestBody TeamModelConfig config) {
        if (config.getRecipientId() == null) {
            return error("请先查询并选择接收用户");
        }
        SysUser recipient = userService.selectUserById(config.getRecipientId());
        if (recipient == null || !"0".equals(recipient.getDelFlag())) {
            return error("接收用户不存在");
        }
        if (config.getIsActive() == null) {
            config.setIsActive(1);
        }
        config.setDeptId(null);
        config.setCreatedBy(SecurityUtils.getUserId());
        return toAjax(modelConfigService.insertTeamModelConfig(config));
    }

    @PutMapping
    @PreAuthorize("@ss.hasPermi('system:model:edit')")
    public AjaxResult edit(@RequestBody TeamModelConfig config) {
        return toAjax(modelConfigService.updateTeamModelConfig(config));
    }

    @DeleteMapping("/{ids}")
    @PreAuthorize("@ss.hasPermi('system:model:remove')")
    public AjaxResult remove(@PathVariable Long[] ids) {
        return toAjax(modelConfigService.deleteTeamModelConfigByIds(ids));
    }

    @DeleteMapping("/personal/{id}")
    public AjaxResult removePersonal(@PathVariable Long id) {
        return toAjax(modelConfigService.deletePersonalModelConfigById(id, SecurityUtils.getUserId()));
    }
}
