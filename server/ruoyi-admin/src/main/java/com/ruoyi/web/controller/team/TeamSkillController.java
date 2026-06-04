package com.ruoyi.web.controller.team;

import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.LocalDate;
import java.util.Base64;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import jakarta.annotation.Resource;
import jakarta.servlet.http.HttpServletResponse;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
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
import com.ruoyi.common.config.RuoYiConfig;
import com.ruoyi.common.core.controller.BaseController;
import com.ruoyi.common.core.domain.AjaxResult;
import com.ruoyi.common.core.domain.model.LoginUser;
import com.ruoyi.common.core.page.TableDataInfo;
import com.ruoyi.common.utils.SecurityUtils;
import com.ruoyi.common.utils.StringUtils;
import com.ruoyi.common.utils.file.FileUtils;
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
    public AjaxResult add(@RequestBody TeamSkillRequest request) {
        TeamSkill skill = request.toTeamSkill();
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
        skill.setCreatedBy(SecurityUtils.getUsername());
        skill.setDeptId(SecurityUtils.getLoginUser().getUser().getDeptId());
        if (StringUtils.isNotEmpty(request.getZipBase64())) {
            try {
                saveZipFile(skill, request.getZipFileName(), request.getZipBase64());
            } catch (Exception e) {
                return error("zip 保存失败: " + e.getMessage());
            }
        }
        return toAjax(teamSkillService.insertTeamSkill(skill));
    }

    @GetMapping("/skills/{id}/zip")
    public void downloadZip(@PathVariable Long id, HttpServletResponse response) throws Exception {
        TeamSkill skill = teamSkillService.selectTeamSkillById(id);
        if (skill == null || StringUtils.isEmpty(skill.getZipFilePath())) {
            response.setStatus(HttpServletResponse.SC_NOT_FOUND);
            return;
        }
        File file = resolveZipFile(skill.getZipFilePath());
        if (!file.exists() || !file.isFile()) {
            response.setStatus(HttpServletResponse.SC_NOT_FOUND);
            return;
        }
        response.setContentType(MediaType.APPLICATION_OCTET_STREAM_VALUE);
        FileUtils.setAttachmentResponseHeader(response,
            StringUtils.isNotEmpty(skill.getZipFileName()) ? skill.getZipFileName() : file.getName());
        FileUtils.writeBytes(file.getAbsolutePath(), response.getOutputStream());
    }

    @PutMapping("/skills")
    public AjaxResult edit(@RequestBody TeamSkill skill) {
        return toAjax(teamSkillService.updateTeamSkill(skill));
    }

    @DeleteMapping("/skills/{ids}")
    public AjaxResult remove(@PathVariable Long[] ids) {
        return toAjax(teamSkillService.deleteTeamSkillByIds(ids));
    }

    private void saveZipFile(TeamSkill skill, String originalName, String zipBase64) throws Exception {
        String fileName = StringUtils.isNotEmpty(originalName) ? originalName : skill.getName() + ".zip";
        if (!fileName.toLowerCase().endsWith(".zip")) {
            throw new IllegalArgumentException("只允许上传 .zip 文件");
        }
        String safeName = fileName.replaceAll("[\\\\/:*?\"<>|\\x00-\\x1F]", "-");
        byte[] bytes = Base64.getDecoder().decode(zipBase64);
        String relativeDir = "team-skills/" + SecurityUtils.getUserId() + "/" + LocalDate.now();
        Path dir = Path.of(RuoYiConfig.getProfile(), relativeDir);
        Files.createDirectories(dir);
        String storedName = UUID.randomUUID().toString().replace("-", "") + "-" + safeName;
        Path target = dir.resolve(storedName).normalize();
        if (!target.startsWith(dir)) {
            throw new IllegalArgumentException("非法文件路径");
        }
        Files.write(target, bytes);
        skill.setZipFileName(safeName);
        skill.setZipFilePath(relativeDir + "/" + storedName);
        skill.setZipFileSize((long) bytes.length);
    }

    private File resolveZipFile(String relativePath) {
        String normalized = relativePath.replace('\\', '/');
        if (normalized.startsWith("/") || normalized.contains("..")) {
            return new File("");
        }
        return Path.of(RuoYiConfig.getProfile(), normalized).normalize().toFile();
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

    public static class TeamSkillRequest {
        private String name;
        private String description;
        private String category;
        private String sourceType;
        private String originAgent;
        private String bodyMd;
        private String compatibleModels;
        private String compatibleAgents;
        private String zipFileName;
        private String zipBase64;

        public TeamSkill toTeamSkill() {
            TeamSkill skill = new TeamSkill();
            skill.setName(name);
            skill.setDescription(description);
            skill.setCategory(category);
            skill.setSourceType(sourceType);
            skill.setOriginAgent(originAgent);
            skill.setBodyMd(bodyMd);
            skill.setCompatibleModels(compatibleModels);
            skill.setCompatibleAgents(compatibleAgents);
            return skill;
        }

        public String getName() { return name; }
        public void setName(String name) { this.name = name; }
        public String getDescription() { return description; }
        public void setDescription(String description) { this.description = description; }
        public String getCategory() { return category; }
        public void setCategory(String category) { this.category = category; }
        public String getSourceType() { return sourceType; }
        public void setSourceType(String sourceType) { this.sourceType = sourceType; }
        public String getOriginAgent() { return originAgent; }
        public void setOriginAgent(String originAgent) { this.originAgent = originAgent; }
        public String getBodyMd() { return bodyMd; }
        public void setBodyMd(String bodyMd) { this.bodyMd = bodyMd; }
        public String getCompatibleModels() { return compatibleModels; }
        public void setCompatibleModels(String compatibleModels) { this.compatibleModels = compatibleModels; }
        public String getCompatibleAgents() { return compatibleAgents; }
        public void setCompatibleAgents(String compatibleAgents) { this.compatibleAgents = compatibleAgents; }
        public String getZipFileName() { return zipFileName; }
        public void setZipFileName(String zipFileName) { this.zipFileName = zipFileName; }
        public String getZipBase64() { return zipBase64; }
        public void setZipBase64(String zipBase64) { this.zipBase64 = zipBase64; }
    }
}
