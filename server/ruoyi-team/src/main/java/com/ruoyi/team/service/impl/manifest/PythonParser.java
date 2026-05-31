package com.ruoyi.team.service.impl.manifest;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class PythonParser implements ManifestParser {
    private static final Pattern REQ_LINE = Pattern.compile("^([A-Za-z0-9_.-]+)\\s*([><=!~]+[^;]+)?\\s*(?:;.*)?$");
    private static final Pattern POETRY_DEP = Pattern.compile("^([A-Za-z0-9_.-]+)\\s*=\\s*[\"']([^\"']+)[\"']");

    @Override
    public String ecosystem() { return "PyPI"; }

    @Override
    public boolean supports(String fileName) {
        String lower = fileName.toLowerCase();
        return lower.equals("requirements.txt") || lower.endsWith("requirements.in")
            || lower.equals("pyproject.toml");
    }

    @Override
    public List<ParsedDependency> parse(String fileName, String content) {
        if (fileName.toLowerCase().endsWith(".toml")) return parsePyprojectToml(content);
        return parseRequirementsTxt(content);
    }

    private List<ParsedDependency> parseRequirementsTxt(String content) {
        List<ParsedDependency> deps = new ArrayList<>();
        for (String line : content.split("\r?\n")) {
            String trimmed = line.trim();
            if (trimmed.isEmpty() || trimmed.startsWith("#") || trimmed.startsWith("-")) continue;
            Matcher m = REQ_LINE.matcher(trimmed);
            if (m.find()) deps.add(new ParsedDependency(m.group(1), m.group(2) != null ? m.group(2).trim() : ""));
        }
        return deps;
    }

    private List<ParsedDependency> parsePyprojectToml(String content) {
        List<ParsedDependency> deps = new ArrayList<>();
        boolean inDeps = false;
        for (String line : content.split("\r?\n")) {
            String trimmed = line.trim();
            if (trimmed.startsWith("[tool.poetry.dependencies]")) { inDeps = true; continue; }
            if (inDeps && trimmed.startsWith("[")) break;
            if (!inDeps || trimmed.isEmpty()) continue;
            Matcher m = POETRY_DEP.matcher(trimmed);
            if (m.find()) deps.add(new ParsedDependency(m.group(1), m.group(2)));
        }
        return deps;
    }
}
