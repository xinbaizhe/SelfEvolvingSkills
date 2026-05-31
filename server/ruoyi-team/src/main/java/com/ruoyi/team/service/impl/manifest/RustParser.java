package com.ruoyi.team.service.impl.manifest;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class RustParser implements ManifestParser {
    private static final Pattern DEP = Pattern.compile("^([A-Za-z0-9_-]+)\\s*=\\s*[\"']([^\"']+)[\"']");

    @Override
    public String ecosystem() { return "crates.io"; }

    @Override
    public boolean supports(String fileName) {
        return fileName.toLowerCase().equals("cargo.toml");
    }

    @Override
    public List<ParsedDependency> parse(String fileName, String content) {
        List<ParsedDependency> deps = new ArrayList<>();
        boolean inDeps = false;
        for (String line : content.split("\r?\n")) {
            String trimmed = line.trim();
            if (trimmed.equals("[dependencies]")) { inDeps = true; continue; }
            if (inDeps && trimmed.startsWith("[")) break;
            if (!inDeps || trimmed.isEmpty()) continue;
            Matcher m = DEP.matcher(trimmed);
            if (m.find()) deps.add(new ParsedDependency(m.group(1), m.group(2)));
        }
        return deps;
    }
}
