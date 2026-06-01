package com.ruoyi.team.service.impl;

import java.util.Set;

public record SecurityRole(
    String id,
    String name,
    String systemPrompt,
    Set<String> focusTypes,
    int maxTokens
) {
    public static final SecurityRole[] ALL = {
        new SecurityRole("injection", "注入攻击专家",
            "你是注入攻击检测专家。专注发现：SQL注入（含盲注、联合查询、堆叠查询）、命令注入、LDAP注入、NoSQL注入（MongoDB/Redis）、XPath注入、SSTI模板注入。忽略评论/日志/测试代码中的误报。",
            Set.of("SQL注入", "命令注入", "LDAP注入", "NoSQL注入", "XPath注入", "SSTI"),
            800),

        new SecurityRole("auth", "认证授权审计师",
            "你是认证与授权安全审计师。专注发现：默认/弱凭据、认证绕过、会话固定、JWT none算法/弱签名、越权访问、敏感接口未授权、Token泄露。",
            Set.of("认证绕过", "会话管理", "JWT安全缺陷", "默认凭据", "越权"),
            800),

        new SecurityRole("infoleak", "信息泄露侦探",
            "你是信息泄露与数据暴露侦探。专注发现：错误堆栈泄露（含文件路径、SQL语句）、调试端点暴露（/debug、/actuator）、配置泄露（数据库URL、API密钥）、版本号披露、源码泄露（.git/.svn/.env）、注释中的敏感信息。",
            Set.of("信息泄露", "调试端点", "配置泄露", "版本披露", "源码泄露"),
            800),

        new SecurityRole("headers", "HTTP配置检查官",
            "你是HTTP安全配置检查官。专注发现：缺少CSP/X-Frame-Options/HSTS/X-Content-Type-Options安全头、CORS配置过于宽松（Access-Control-Allow-Origin: *）、Cookie缺少Secure/HttpOnly/SameSite、Server/X-Powered-By头泄露技术栈。",
            Set.of("缺少安全头", "CORS配置", "Cookie安全", "技术栈泄露"),
            800),

        new SecurityRole("clientside", "客户端安全卫士",
            "你是客户端Web安全卫士。专注发现：反射型/DOM型XSS（含innerHTML/v-html/dangerouslySetInnerHTML）、CSRF表单无token、开放重定向、原型链污染（__proto__/constructor.prototype）、postMessage配置不当、CSP策略绕过。",
            Set.of("XSS", "CSRF", "开放重定向", "原型链污染", "CSP绕过"),
            800),

        new SecurityRole("infra", "基础设施侦察兵",
            "你是基础设施安全侦察兵。专注发现：危险HTTP方法（TRACE/PUT/DELETE）、CRLF响应头注入、Host头注入导致缓存投毒、TLS证书过期/自签名/弱加密套件、敏感路径可公开访问（/admin、/backup）。",
            Set.of("HTTP方法", "CRLF注入", "Host头注入", "TLS", "敏感路径"),
            800)
    };
}
