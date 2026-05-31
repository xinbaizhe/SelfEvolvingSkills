package com.ruoyi.team.service.impl.manifest;

import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;

public class PhpParser implements ManifestParser {
    private static final ObjectMapper mapper = new ObjectMapper();

    @Override
    public String ecosystem() { return "Packagist"; }

    @Override
    public boolean supports(String fileName) {
        return fileName.toLowerCase().equals("composer.json");
    }

    @Override
    public List<ParsedDependency> parse(String fileName, String content) {
        List<ParsedDependency> deps = new ArrayList<>();
        try {
            JsonNode root = mapper.readTree(content);
            collectDeps(root.path("require"), deps);
            collectDeps(root.path("require-dev"), deps);
        } catch (Exception ignored) {}
        return deps;
    }

    private void collectDeps(JsonNode node, List<ParsedDependency> deps) {
        if (!node.isObject()) return;
        for (Iterator<String> it = node.fieldNames(); it.hasNext(); ) {
            String name = it.next();
            deps.add(new ParsedDependency(name, node.path(name).asText("")));
        }
    }
}
