package com.ruoyi.team.service.impl.manifest;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class DotNetParser implements ManifestParser {
    private static final Pattern PKG_REF = Pattern.compile(
        "<PackageReference\\s+Include=\"([^\"]+)\"\\s+Version=\"([^\"]+)\"");

    @Override
    public String ecosystem() { return "NuGet"; }

    @Override
    public boolean supports(String fileName) {
        return fileName.toLowerCase().endsWith(".csproj");
    }

    @Override
    public List<ParsedDependency> parse(String fileName, String content) {
        List<ParsedDependency> deps = new ArrayList<>();
        Matcher m = PKG_REF.matcher(content);
        while (m.find()) {
            deps.add(new ParsedDependency(m.group(1), m.group(2)));
        }
        return deps;
    }
}
