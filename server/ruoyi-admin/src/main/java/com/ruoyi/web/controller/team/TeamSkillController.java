package com.ruoyi.web.controller.team;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import jakarta.annotation.Resource;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.security.authentication.AuthenticationManager;
import org.springframework.security.authentication.BadCredentialsException;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.Authentication;
import org.springframework.security.access.prepost.PreAuthorize;
import jakarta.servlet.http.HttpServletRequest;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.PutMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;
import com.ruoyi.common.annotation.Anonymous;
import com.ruoyi.common.core.controller.BaseController;
import com.ruoyi.common.core.domain.AjaxResult;
import com.ruoyi.common.core.domain.model.LoginUser;
import com.ruoyi.common.core.page.TableDataInfo;
import com.ruoyi.common.utils.SecurityUtils;
import com.ruoyi.common.utils.StringUtils;
import com.ruoyi.framework.security.context.AuthenticationContextHolder;
import com.ruoyi.framework.web.service.TokenService;
import com.ruoyi.common.core.domain.TreeSelect;
import com.ruoyi.common.core.domain.entity.SysDept;
import com.ruoyi.common.core.domain.entity.SysRole;
import com.ruoyi.system.domain.SysPost;
import com.ruoyi.system.service.ISysDeptService;
import com.ruoyi.system.service.ISysPostService;
import com.ruoyi.system.service.ISysRoleService;
import com.ruoyi.team.domain.TeamModelConfig;
import com.ruoyi.team.domain.TeamSkill;
import com.ruoyi.team.service.ITeamModelConfigService;
import com.ruoyi.team.service.ITeamSkillService;

@RestController
@RequestMapping("/api/team")
@PreAuthorize("isAuthenticated()")
public class TeamSkillController extends BaseController {

    @Autowired
    private ITeamSkillService teamSkillService;

    @Autowired
    private ITeamModelConfigService modelConfigService;

    @Autowired
    private TokenService tokenService;

    @Autowired
    private ISysDeptService deptService;

    @Autowired
    private ISysRoleService roleService;

    @Autowired
    private ISysPostService postService;

    @Resource
    private AuthenticationManager authenticationManager;

    @Anonymous
    @PreAuthorize("permitAll()")
    @GetMapping("/ping")
    public AjaxResult ping() {
        Map<String, Object> result = new HashMap<>();
        result.put("status", "ok");
        result.put("version", "2.0.1");
        return success(result);
    }

    @Anonymous
    @PreAuthorize("permitAll()")
    @PostMapping("/login")
    public AjaxResult login(@RequestBody Map<String, String> body) {
        String username = body.get("username");
        String password = body.get("password");

        if (username == null || password == null || username.isBlank() || password.isBlank()) {
            return error("用户名和密码不能为空");
        }

        Authentication authentication = null;
        try {
            UsernamePasswordAuthenticationToken authToken =
                new UsernamePasswordAuthenticationToken(username, password);
            AuthenticationContextHolder.setContext(authToken);
            authentication = authenticationManager.authenticate(authToken);
            LoginUser loginUser = (LoginUser) authentication.getPrincipal();

            String token = tokenService.createToken(loginUser);

            Map<String, Object> result = new HashMap<>();
            result.put("token", token);
            result.put("refresh_token", "");
            result.put("username", loginUser.getUser().getUserName());
            result.put("user_id", loginUser.getUser().getUserId());
            result.put("dept_name", loginUser.getUser().getDept() != null
                ? loginUser.getUser().getDept().getDeptName() : "");
            result.put("dept_id", loginUser.getUser().getDept() != null
                ? loginUser.getUser().getDept().getDeptId() : 0);
            return success(result);
        } catch (BadCredentialsException e) {
            return error("用户名或密码错误");
        } catch (Exception e) {
            return error("登录失败: " + e.getMessage());
        } finally {
            AuthenticationContextHolder.clearContext();
        }
    }

    @PostMapping("/logout")
    public AjaxResult logout(HttpServletRequest request) {
        String token = request.getHeader("Authorization");
        if (StringUtils.isNotEmpty(token) && token.startsWith("Bearer ")) {
            token = token.substring(7);
            tokenService.delLoginUser(token);
        }
        return success();
    }

    @PostMapping("/refresh")
    public AjaxResult refresh() {
        LoginUser loginUser = SecurityUtils.getLoginUser();
        String token = tokenService.createToken(loginUser);
        Map<String, Object> result = new HashMap<>();
        result.put("token", token);
        return success(result);
    }

    @GetMapping("/profile")
    public AjaxResult profile() {
        LoginUser loginUser = SecurityUtils.getLoginUser();
        Long deptId = loginUser.getUser().getDeptId();
        List<TeamModelConfig> models = modelConfigService.selectAvailableModelConfigs(
            deptId,
            loginUser.getUser().getUserId(),
            null
        );

        Map<String, Object> result = new HashMap<>();
        result.put("username", loginUser.getUser().getUserName());
        result.put("user_id", loginUser.getUser().getUserId());
        result.put("dept_name", loginUser.getUser().getDept() != null
            ? loginUser.getUser().getDept().getDeptName() : "");
        result.put("dept_id", deptId);
        result.put("available_models", models);
        return success(result);
    }

    @GetMapping("/skills/list")
    public TableDataInfo list(TeamSkill skill,
            @RequestParam(required = false) Long roleId,
            @RequestParam(required = false) Long postId) {
        startPage();
        if (roleId != null) {
            skill.getParams().put("roleId", roleId);
        }
        if (postId != null) {
            skill.getParams().put("postId", postId);
        }
        List<TeamSkill> list = teamSkillService.selectTeamSkillList(skill);
        return getDataTable(list);
    }

    @GetMapping("/skills/{id}")
    public AjaxResult getInfo(@PathVariable Long id) {
        TeamSkill skill = teamSkillService.selectTeamSkillById(id);
        if (skill != null) {
            teamSkillService.incrementUsageCount(id);
        }
        return success(skill);
    }

    @PostMapping("/skills")
    public AjaxResult add(@RequestBody TeamSkill skill) {
        if (StringUtils.isEmpty(skill.getSourceType())) {
            skill.setSourceType("skill");
        }
        if (StringUtils.isEmpty(skill.getStatus())) {
            skill.setStatus("published");
        }
        if (skill.getVersion() == null) {
            skill.setVersion(1);
        }
        skill.setAuthorId(SecurityUtils.getUserId());
        skill.setDeptId(SecurityUtils.getLoginUser().getUser().getDeptId());
        return toAjax(teamSkillService.insertTeamSkill(skill));
    }

    @PutMapping("/skills")
    public AjaxResult edit(@RequestBody TeamSkill skill) {
        return toAjax(teamSkillService.updateTeamSkill(skill));
    }

    @DeleteMapping("/skills/{ids}")
    public AjaxResult remove(@PathVariable Long[] ids) {
        return toAjax(teamSkillService.deleteTeamSkillByIds(ids));
    }

    @GetMapping("/deptTree")
    public AjaxResult deptTree() {
        SysDept dept = new SysDept();
        dept.setStatus("0");
        List<TreeSelect> tree = deptService.selectDeptTreeList(dept);
        return success(tree);
    }

    @GetMapping("/roles")
    public AjaxResult roles() {
        List<SysRole> roles = roleService.selectRoleAll();
        return success(roles);
    }

    @GetMapping("/posts")
    public AjaxResult posts() {
        List<SysPost> posts = postService.selectPostAll();
        return success(posts);
    }
}
