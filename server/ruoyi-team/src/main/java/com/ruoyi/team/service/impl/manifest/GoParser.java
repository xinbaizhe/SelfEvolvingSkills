package com.ruoyi.team.service.impl.manifest;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class GoParser implements ManifestParser {
    private static final Pattern REQUIRE = Pattern.compile("^\\t([^\\s]+)\\s+(v[^\\s]+)");

    @Override
    public String ecosystem() { return "Go"; }

    @Override
    public boolean supports(String fileName) {
        return fileName.toLowerCase().equals("go.mod");
    }

    @Override
    public List<ParsedDependency> parse(String fileName, String content) {
        List<ParsedDependency> deps = new ArrayList<>();
        boolean inRequire = false;
        for (String line : content.split("\r?\n")) {
            String trimmed = line.trim();
            if (trimmed.startsWith("require (")) { inRequire = true; continue; }
            if (inRequire && trimmed.equals(")")) { inRequire = false; continue; }
            if (!inRequire && trimmed.startsWith("require ")) {
                Matcher m = REQUIRE.matcher(trimmed);
                if (m.find()) deps.add(new ParsedDependency(m.group(1), m.group(2)));
                continue;
            }
            if (inRequire) {
                Matcher m = REQUIRE.matcher(line);
                if (m.find()) deps.add(new ParsedDependency(m.group(1), m.group(2)));
            }
        }
        return deps;
    }
}
