package com.ruoyi.team.service.impl;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import com.ruoyi.team.domain.DepFinding;
import com.ruoyi.team.domain.DepMonitor;
import com.ruoyi.team.mapper.DepFindingMapper;
import com.ruoyi.team.mapper.DepMonitorMapper;
import com.ruoyi.team.service.IVulnMonitorService;
import com.ruoyi.team.service.impl.manifest.ManifestParser;
import com.ruoyi.team.service.impl.manifest.ManifestParser.ParsedDependency;
import com.ruoyi.team.service.impl.manifest.NpmParser;
import com.ruoyi.team.service.impl.manifest.PythonParser;
import com.ruoyi.team.service.impl.manifest.MavenParser;
import com.ruoyi.team.service.impl.manifest.GradleParser;
import com.ruoyi.team.service.impl.manifest.GoParser;
import com.ruoyi.team.service.impl.manifest.RustParser;
import com.ruoyi.team.service.impl.manifest.DotNetParser;
import com.ruoyi.team.service.impl.manifest.PhpParser;

@Service
public class VulnMonitorServiceImpl implements IVulnMonitorService {

    private static final Logger log = LoggerFactory.getLogger(VulnMonitorServiceImpl.class);
    private static final String OSV_QUERYBATCH_URL = "https://api.osv.dev/v1/querybatch";
    private static final ObjectMapper mapper = new ObjectMapper();

    private static final ManifestParser[] PARSERS = {
        new NpmParser(), new PythonParser(), new MavenParser(), new GradleParser(),
        new GoParser(), new RustParser(), new DotNetParser(), new PhpParser()
    };

    @Autowired
    private DepMonitorMapper depMonitorMapper;

    @Autowired
    private DepFindingMapper depFindingMapper;

    private final HttpClient httpClient = HttpClient.newBuilder()
        .connectTimeout(Duration.ofSeconds(10))
        .build();

    @Override
    @Transactional
    public List<DepMonitor> uploadManifest(String name, List<Map<String, String>> files, Long userId, Long deptId) {
        depMonitorMapper.deleteDepMonitorByUserIdAndName(userId, name);

        List<ParsedDependency> allDeps = new ArrayList<>();
        for (Map<String, String> file : files) {
            String fileName = file.get("name");
            String content = file.get("content");
            if (fileName == null || content == null) continue;
            for (ManifestParser parser : PARSERS) {
                if (parser.supports(fileName)) {
                    allDeps.addAll(parser.parse(fileName, content));
                    break;
                }
            }
        }

        List<DepMonitor> saved = new ArrayList<>();
        for (ParsedDependency pd : allDeps) {
            String eco = detectEcosystem(pd, allDeps);
            if (eco == null) continue;
            DepMonitor dm = new DepMonitor();
            dm.setUserId(userId);
            dm.setDeptId(deptId);
            dm.setName(name);
            dm.setEcosystem(eco);
            dm.setPackageName(pd.packageName());
            dm.setVersion(pd.version().isEmpty() ? null : pd.version());
            dm.setVulnCount(0);
            dm.setPoisoningCount(0);
            depMonitorMapper.insertDepMonitor(dm);
            saved.add(dm);
        }

        queryAndSaveFindings(saved);
        return saved;
    }

    @Override
    public List<DepMonitor> getSnapshots(Long userId) {
        List<DepMonitor> all = depMonitorMapper.selectByUserId(userId);
        Map<String, DepMonitor> latest = new LinkedHashMap<>();
        for (DepMonitor dm : all) {
            latest.putIfAbsent(dm.getName(), dm);
        }
        List<DepMonitor> result = new ArrayList<>();
        for (DepMonitor dm : latest.values()) {
            List<DepMonitor> deps = depMonitorMapper.selectByUserIdAndName(userId, dm.getName());
            int totalVuln = 0, totalPoisoning = 0;
            for (DepMonitor d : deps) {
                totalVuln += d.getVulnCount() != null ? d.getVulnCount() : 0;
                totalPoisoning += d.getPoisoningCount() != null ? d.getPoisoningCount() : 0;
            }
            dm.setVulnCount(totalVuln);
            dm.setPoisoningCount(totalPoisoning);
            result.add(dm);
        }
        return result;
    }

    @Override
    public List<DepMonitor> getDependencies(Long userId, String name) {
        return depMonitorMapper.selectByUserIdAndName(userId, name);
    }

    @Override
    public List<DepFinding> getFindings(Long depId) {
        return depFindingMapper.selectByDepId(depId);
    }

    @Override
    @Transactional
    public List<DepFinding> refreshSnapshot(Long snapshotId, Long userId) {
        DepMonitor dep = depMonitorMapper.selectByUserId(userId).stream()
            .filter(d -> d.getId().equals(snapshotId)).findFirst().orElse(null);
        if (dep == null) return List.of();

        List<DepMonitor> siblings = depMonitorMapper.selectByUserIdAndName(userId, dep.getName());
        depFindingMapper.deleteByDepId(dep.getId());
        queryAndSaveFindings(siblings);
        return depFindingMapper.selectByDepId(dep.getId());
    }

    @Override
    @Transactional
    public void deleteSnapshot(Long snapshotId, Long userId) {
        DepMonitor dep = depMonitorMapper.selectByUserId(userId).stream()
            .filter(d -> d.getId().equals(snapshotId)).findFirst().orElse(null);
        if (dep == null) return;
        List<DepMonitor> siblings = depMonitorMapper.selectByUserIdAndName(userId, dep.getName());
        for (DepMonitor s : siblings) {
            depFindingMapper.deleteByDepId(s.getId());
            depMonitorMapper.deleteDepMonitorById(s.getId());
        }
    }

    private void queryAndSaveFindings(List<DepMonitor> deps) {
        for (int i = 0; i < deps.size(); i += 100) {
            int end = Math.min(i + 100, deps.size());
            List<DepMonitor> batch = deps.subList(i, end);
            try {
                JsonNode response = queryOsvBatch(batch);
                if (response == null) continue;
                for (int j = 0; j < batch.size(); j++) {
                    DepMonitor dm = batch.get(j);
                    JsonNode results = response.path("results").path(j);
                    JsonNode vulns = results.path("vulns");
                    int vulnCount = 0, poisoningCount = 0;
                    List<DepFinding> findings = new ArrayList<>();
                    if (vulns.isArray()) {
                        for (JsonNode vuln : vulns) {
                            String cveId = firstAliasCve(vuln.path("aliases"));
                            boolean isPoisoning = isOsvPoisoning(vuln);
                            DepFinding df = new DepFinding();
                            df.setDepId(dm.getId());
                            df.setCveId(cveId != null ? cveId : text(vuln, "id"));
                            df.setTitle(text(vuln, "summary"));
                            df.setSeverity(isPoisoning ? "CRITICAL" : "MEDIUM");
                            df.setIsPoisoning(isPoisoning);
                            df.setReferenceUrl(extractOsvRef(vuln));
                            findings.add(df);
                            if (isPoisoning) poisoningCount++; else vulnCount++;
                        }
                    }
                    if (!findings.isEmpty()) {
                        depFindingMapper.batchInsertDepFindings(findings);
                    }
                    dm.setVulnCount(vulnCount);
                    dm.setPoisoningCount(poisoningCount);
                    depMonitorMapper.updateDepMonitorCounts(dm);
                }
            } catch (Exception e) {
                log.warn("OSV batch query failed for batch {}: {}", i, e.getMessage());
            }
            if (end < deps.size()) sleep(1000);
        }
    }

    private JsonNode queryOsvBatch(List<DepMonitor> deps) throws Exception {
        ObjectNode body = mapper.createObjectNode();
        ArrayNode queries = mapper.createArrayNode();
        for (DepMonitor dm : deps) {
            ObjectNode query = mapper.createObjectNode();
            ObjectNode pkg = mapper.createObjectNode();
            pkg.put("name", dm.getPackageName());
            pkg.put("ecosystem", dm.getEcosystem());
            query.set("package", pkg);
            if (dm.getVersion() != null && !dm.getVersion().isBlank()) {
                query.put("version", dm.getVersion());
            }
            queries.add(query);
        }
        body.set("queries", queries);

        HttpRequest request = HttpRequest.newBuilder()
            .uri(URI.create(OSV_QUERYBATCH_URL))
            .timeout(Duration.ofSeconds(60))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .POST(HttpRequest.BodyPublishers.ofString(mapper.writeValueAsString(body)))
            .build();
        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() < 200 || response.statusCode() >= 300) {
            throw new IllegalStateException("HTTP " + response.statusCode());
        }
        return mapper.readTree(response.body());
    }

    private String detectEcosystem(ParsedDependency pd, List<ParsedDependency> allDeps) {
        String name = pd.packageName();
        if (name.contains(":")) return "Maven";
        if (name.contains("/")) {
            if (name.startsWith("@")) return "npm";
            return "Go";
        }
        return "PyPI";
    }

    private static String text(JsonNode node, String field) {
        String value = node.path(field).asText("");
        return value.isBlank() ? null : value;
    }

    private static String firstAliasCve(JsonNode aliases) {
        if (aliases.isArray()) {
            for (JsonNode a : aliases) {
                String v = a.asText("");
                if (v.startsWith("CVE-")) return v;
            }
        }
        return null;
    }

    private static boolean isOsvPoisoning(JsonNode vuln) {
        JsonNode dbSpecific = vuln.path("database_specific");
        if (!dbSpecific.path("malicious_type").isMissingNode()) return true;
        JsonNode tags = vuln.path("tags");
        if (tags.isArray()) {
            for (JsonNode t : tags) {
                String tag = t.asText("").toLowerCase();
                if (tag.contains("malicious") || tag.contains("malware")
                    || tag.contains("backdoor") || tag.contains("typosquat")) return true;
            }
        }
        return false;
    }

    private static String extractOsvRef(JsonNode vuln) {
        JsonNode refs = vuln.path("references");
        if (refs.isArray()) {
            for (JsonNode r : refs) {
                if ("ADVISORY".equalsIgnoreCase(text(r, "type"))) {
                    String url = text(r, "url");
                    if (url != null) return url;
                }
            }
        }
        String id = text(vuln, "id");
        return id != null ? "https://osv.dev/" + id : null;
    }

    private static void sleep(long ms) {
        try { Thread.sleep(ms); } catch (InterruptedException ignored) {}
    }
}
