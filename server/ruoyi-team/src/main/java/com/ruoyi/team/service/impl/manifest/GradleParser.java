package com.ruoyi.team.service.impl.manifest;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class GradleParser implements ManifestParser {
    private static final Pattern DEP_LINE = Pattern.compile(
        "(?:implementation|api|compile|runtimeOnly|classpath|testImplementation)\\s*['\"]([^'\"]+)['\"]");

    @Override
    public String ecosystem() { return "Maven"; }

    @Override
    public boolean supports(String fileName) {
        String lower = fileName.toLowerCase();
        return lower.equals("build.gradle") || lower.equals("build.gradle.kts");
    }

    @Override
    public List<ParsedDependency> parse(String fileName, String content) {
        List<ParsedDependency> deps = new ArrayList<>();
        Matcher m = DEP_LINE.matcher(content);
        while (m.find()) {
            String raw = m.group(1);
            String[] parts = raw.split(":");
            if (parts.length >= 2) {
                String name = parts[0] + ":" + parts[1];
                String version = parts.length >= 3 ? parts[2] : "";
                deps.add(new ParsedDependency(name, version));
            }
        }
        return deps;
    }
}
