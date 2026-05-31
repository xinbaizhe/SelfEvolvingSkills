package com.ruoyi.team.service.impl.manifest;

import java.util.List;

public interface ManifestParser {
    String ecosystem();
    boolean supports(String fileName);
    List<ParsedDependency> parse(String fileName, String content);

    record ParsedDependency(String packageName, String version) {}
}
