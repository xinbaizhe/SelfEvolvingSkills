package com.ruoyi.team.service.impl;

import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.OffsetDateTime;
import java.time.ZoneOffset;
import java.time.format.DateTimeFormatter;
import java.util.List;

import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Service;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.ruoyi.team.domain.VulnIntel;
import com.ruoyi.team.mapper.VulnIntelMapper;
import com.ruoyi.team.service.IVulnIntelService;

@Service
public class VulnIntelServiceImpl implements IVulnIntelService {
    private static final String CISA_KEV_URL = "https://www.cisa.gov/sites/default/files/feeds/known_exploited_vulnerabilities.json";
    private static final String NVD_CVE_URL = "https://services.nvd.nist.gov/rest/json/cves/2.0";
    private static final String GITHUB_ADVISORIES_URL = "https://api.github.com/advisories";
    private static final String OSV_VULNS_URL = "https://api.osv.dev/v1/vulns";
    private static final DateTimeFormatter NVD_DATE_TIME_FORMAT = DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm:ss.SSSxxx");
    private static final ObjectMapper mapper = new ObjectMapper();

    @Autowired
    private VulnIntelMapper vulnIntelMapper;

    private final HttpClient httpClient = HttpClient.newBuilder()
        .connectTimeout(Duration.ofSeconds(10))
        .build();

    @Override
    public List<VulnIntel> list(String vulnType, String severity, String keyword, String startDate, String endDate) {
        return vulnIntelMapper.selectVulnIntelList(normalizeVulnType(vulnType), normalizeSeverity(severity), keyword, startDate, endDate);
    }

    @Override
//    @Scheduled(initialDelay = 60_000, fixedDelay = 1_800_000)
    public int syncPublicIntel() {
        int count = 0;
        count += syncCisaKev();
        count += syncNvdRecentCves();
        count += syncGithubAdvisories();
        count += syncOsvDev();
        return count;
    }

    private int syncCisaKev() {
        try {
            JsonNode root = getJson(CISA_KEV_URL);
            JsonNode vulnerabilities = root.path("vulnerabilities");
            if (!vulnerabilities.isArray()) {
                return 0;
            }
            int count = 0;
            for (JsonNode item : vulnerabilities) {
                VulnIntel intel = new VulnIntel();
                intel.setSource("CISA KEV");
                intel.setCveId(text(item, "cveID"));
                intel.setTitle(text(item, "vulnerabilityName"));
                intel.setVulnType(inferVulnType(intel.getTitle(), text(item, "shortDescription")));
                intel.setSeverity("HIGH");
                intel.setVendorProject(text(item, "vendorProject"));
                intel.setProduct(text(item, "product"));
                intel.setDescription(text(item, "shortDescription"));
                intel.setReferenceUrl(CISA_KEV_URL);
                intel.setPublishedAt(parseDate(text(item, "dateAdded")));
                intel.setUpdatedAt(parseDate(text(item, "dueDate")));
                count += upsertIfValid(intel);
            }
            return count;
        } catch (Exception ignored) {
            return 0;
        }
    }

    private int syncNvdRecentCves() {
        try {
            OffsetDateTime end = OffsetDateTime.now(ZoneOffset.UTC);
            OffsetDateTime start = end.minusDays(30);
            String url = NVD_CVE_URL
                + "?lastModStartDate=" + encode(NVD_DATE_TIME_FORMAT.format(start))
                + "&lastModEndDate=" + encode(NVD_DATE_TIME_FORMAT.format(end))
                + "&resultsPerPage=100";
            JsonNode root = getJson(url);
            JsonNode vulnerabilities = root.path("vulnerabilities");
            if (!vulnerabilities.isArray()) {
                return 0;
            }
            int count = 0;
            for (JsonNode item : vulnerabilities) {
                JsonNode cve = item.path("cve");
                String cveId = text(cve, "id");
                String description = firstDescription(cve.path("descriptions"));
                VulnIntel intel = new VulnIntel();
                intel.setSource("NVD");
                intel.setCveId(cveId);
                intel.setTitle(cveId);
                intel.setVulnType(inferVulnType(cveId, description));
                intel.setSeverity(nvdSeverity(cve.path("metrics")));
                intel.setVendorProject(firstCpePart(cve.path("configurations"), 3));
                intel.setProduct(firstCpePart(cve.path("configurations"), 4));
                intel.setDescription(description);
                intel.setReferenceUrl(cveId == null ? NVD_CVE_URL : "https://nvd.nist.gov/vuln/detail/" + cveId);
                intel.setPublishedAt(parseDate(text(cve, "published")));
                intel.setUpdatedAt(parseDate(text(cve, "lastModified")));
                count += upsertIfValid(intel);
            }
            return count;
        } catch (Exception ignored) {
            return 0;
        }
    }

    private int syncGithubAdvisories() {
        try {
            JsonNode advisories = getJson(GITHUB_ADVISORIES_URL + "?per_page=100&sort=updated&direction=desc");
            if (!advisories.isArray()) {
                return 0;
            }
            int count = 0;
            for (JsonNode item : advisories) {
                VulnIntel intel = new VulnIntel();
                intel.setSource("GitHub Advisory");
                intel.setCveId(firstIdentifier(item.path("identifiers"), "CVE", text(item, "ghsa_id")));
                intel.setTitle(text(item, "summary"));
                intel.setVulnType(inferVulnType(text(item, "summary"), text(item, "description")));
                intel.setSeverity(normalizeSeverity(text(item, "severity")));
                intel.setVendorProject("GitHub");
                intel.setProduct(firstGithubPackage(item.path("vulnerabilities")));
                intel.setDescription(text(item, "description"));
                intel.setReferenceUrl(text(item, "html_url"));
                intel.setPublishedAt(parseDate(text(item, "published_at")));
                intel.setUpdatedAt(parseDate(text(item, "updated_at")));
                count += upsertIfValid(intel);
            }
            return count;
        } catch (Exception ignored) {
            return 0;
        }
    }

    private int syncOsvDev() {
        try {
            OffsetDateTime since = OffsetDateTime.now(ZoneOffset.UTC).minusDays(2);
            String url = OSV_VULNS_URL + "?modified_since=" + encode(since.toString());
            JsonNode root = getJson(url);
            if (!root.isArray()) return 0;
            int count = 0;
            for (JsonNode item : root) {
                boolean poisoning = isOsvPoisoning(item);
                VulnIntel intel = new VulnIntel();
                intel.setSource("OSV");
                intel.setCveId(extractOsvCve(item));
                intel.setTitle(text(item, "summary"));
                intel.setEcosystem(extractOsvEcosystem(item));
                intel.setIsPoisoning(poisoning);
                intel.setVulnType(poisoning ? "供应链投毒" : inferVulnType(text(item, "summary"), text(item, "details")));
                intel.setSeverity(inferOsvSeverity(item));
                intel.setVendorProject(extractOsvVendor(item));
                intel.setProduct(extractOsvProduct(item));
                intel.setDescription(text(item, "details"));
                intel.setReferenceUrl(extractOsvReferenceUrl(item));
                intel.setAliases(extractOsvAliases(item));
                intel.setPublishedAt(parseDate(text(item, "published")));
                intel.setUpdatedAt(parseDate(text(item, "modified")));
                count += upsertIfValid(intel);
            }
            return count;
        } catch (Exception ignored) {
            return 0;
        }
    }

    private JsonNode getJson(String url) throws Exception {
        HttpRequest request = HttpRequest.newBuilder()
            .uri(URI.create(url))
            .timeout(Duration.ofSeconds(30))
            .header("Accept", "application/json")
            .header("User-Agent", "SelfEvolvingSkills-VulnIntel")
            .GET()
            .build();
        HttpResponse<String> response = httpClient.send(request, HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() < 200 || response.statusCode() >= 300) {
            throw new IllegalStateException("HTTP " + response.statusCode());
        }
        return mapper.readTree(response.body());
    }

    private int upsertIfValid(VulnIntel intel) {
        if (intel.getCveId() == null || intel.getCveId().isBlank()) {
            return 0;
        }
        return vulnIntelMapper.upsertVulnIntel(intel);
    }

    // ---- OSV helpers ----

    private static String extractOsvCve(JsonNode item) {
        JsonNode aliases = item.path("aliases");
        if (aliases.isArray()) {
            for (JsonNode alias : aliases) {
                String value = alias.asText("");
                if (value.startsWith("CVE-")) return value;
            }
        }
        return text(item, "id");
    }

    private static String extractOsvEcosystem(JsonNode item) {
        JsonNode affected = item.path("affected");
        if (affected.isArray() && !affected.isEmpty()) {
            JsonNode pkg = affected.get(0).path("package");
            return text(pkg, "ecosystem");
        }
        return null;
    }

    private static String extractOsvVendor(JsonNode item) {
        JsonNode affected = item.path("affected");
        if (affected.isArray() && !affected.isEmpty()) {
            JsonNode pkg = affected.get(0).path("package");
            String ecosystem = text(pkg, "ecosystem");
            String name = text(pkg, "name");
            if (ecosystem != null && name != null) return ecosystem;
            if (name != null) {
                int idx = name.indexOf('/');
                return idx > 0 ? name.substring(0, idx) : name;
            }
        }
        return "OSV";
    }

    private static String extractOsvProduct(JsonNode item) {
        JsonNode affected = item.path("affected");
        if (affected.isArray() && !affected.isEmpty()) {
            JsonNode pkg = affected.get(0).path("package");
            String name = text(pkg, "name");
            if (name != null) {
                int idx = name.indexOf('/');
                return idx > 0 ? name.substring(idx + 1) : name;
            }
        }
        return null;
    }

    private static String extractOsvReferenceUrl(JsonNode item) {
        JsonNode refs = item.path("references");
        if (refs.isArray()) {
            for (JsonNode ref : refs) {
                String type = text(ref, "type");
                String url = text(ref, "url");
                if ("ADVISORY".equalsIgnoreCase(type) && url != null) return url;
            }
        }
        String id = text(item, "id");
        return id != null ? "https://osv.dev/" + id : null;
    }

    private static String extractOsvAliases(JsonNode item) {
        JsonNode aliases = item.path("aliases");
        if (!aliases.isArray() || aliases.isEmpty()) return null;
        java.util.List<String> aliasList = new java.util.ArrayList<>();
        for (JsonNode a : aliases) {
            String val = a.asText("");
            if (!val.isBlank()) aliasList.add(val);
        }
        try {
            return aliasList.isEmpty() ? null : mapper.writeValueAsString(aliasList);
        } catch (Exception ignored) {
            return null;
        }
    }

    private static boolean isOsvPoisoning(JsonNode item) {
        String dbSpecificStr = text(item.path("database_specific"), "malicious_type");
        if (dbSpecificStr != null) return true;
        JsonNode tags = item.path("tags");
        if (tags.isArray()) {
            for (JsonNode t : tags) {
                String tag = t.asText("").toLowerCase();
                if (tag.contains("malicious") || tag.contains("malware")
                    || tag.contains("backdoor") || tag.contains("typosquat")
                    || tag.contains("spam")) return true;
            }
        }
        return false;
    }

    private static String inferOsvSeverity(JsonNode item) {
        if (isOsvPoisoning(item)) return "CRITICAL";
        JsonNode severity = item.path("severity");
        if (!severity.isMissingNode()) {
            for (JsonNode s : severity) {
                String score = text(s, "score");
                if (score != null) {
                    try {
                        double cvss = Double.parseDouble(score);
                        if (cvss >= 9.0) return "CRITICAL";
                        if (cvss >= 7.0) return "HIGH";
                        if (cvss >= 4.0) return "MEDIUM";
                        return "LOW";
                    } catch (NumberFormatException ignored) {}
                }
            }
        }
        return "MEDIUM";
    }

    // ---- Common helpers ----

    private static String text(JsonNode node, String field) {
        String value = node.path(field).asText("");
        return value.isBlank() ? null : value;
    }

    private static String encode(String value) {
        return URLEncoder.encode(value, StandardCharsets.UTF_8);
    }

    private static LocalDateTime parseDate(String value) {
        if (value == null || value.isBlank()) return null;
        try {
            return LocalDate.parse(value).atStartOfDay();
        } catch (Exception ignored) {
            try {
                return OffsetDateTime.parse(value).toLocalDateTime();
            } catch (Exception ignoredAgain) {
                return null;
            }
        }
    }

    private static String firstDescription(JsonNode descriptions) {
        if (!descriptions.isArray()) return null;
        String fallback = null;
        for (JsonNode item : descriptions) {
            String value = text(item, "value");
            if (value == null) continue;
            if (fallback == null) fallback = value;
            if ("en".equalsIgnoreCase(text(item, "lang"))) return value;
        }
        return fallback;
    }

    private static String nvdSeverity(JsonNode metrics) {
        String severity = firstMetricSeverity(metrics.path("cvssMetricV31"));
        if (severity == null) severity = firstMetricSeverity(metrics.path("cvssMetricV30"));
        if (severity == null) severity = firstMetricSeverity(metrics.path("cvssMetricV2"));
        return severity == null ? "MEDIUM" : normalizeSeverity(severity);
    }

    private static String firstMetricSeverity(JsonNode metrics) {
        if (!metrics.isArray() || metrics.isEmpty()) return null;
        String severity = text(metrics.get(0), "baseSeverity");
        if (severity != null) return severity;
        return text(metrics.get(0).path("cvssData"), "baseSeverity");
    }

    private static String firstCpePart(JsonNode configurations, int index) {
        if (!configurations.isArray()) return null;
        for (JsonNode configuration : configurations) {
            String value = firstCpePartFromNodes(configuration.path("nodes"), index);
            if (value != null) return value;
        }
        return null;
    }

    private static String firstCpePartFromNodes(JsonNode nodes, int index) {
        if (!nodes.isArray()) return null;
        for (JsonNode node : nodes) {
            JsonNode matches = node.path("cpeMatch");
            if (matches.isArray()) {
                for (JsonNode match : matches) {
                    String criteria = text(match, "criteria");
                    String part = cpePart(criteria, index);
                    if (part != null) return part;
                }
            }
            String nested = firstCpePartFromNodes(node.path("nodes"), index);
            if (nested != null) return nested;
        }
        return null;
    }

    private static String cpePart(String criteria, int index) {
        if (criteria == null) return null;
        String[] parts = criteria.split(":");
        if (parts.length <= index || "*".equals(parts[index]) || "-".equals(parts[index])) return null;
        return parts[index].replace("\\_", "_").replace('_', ' ');
    }

    private static String firstIdentifier(JsonNode identifiers, String type, String fallback) {
        if (identifiers.isArray()) {
            for (JsonNode item : identifiers) {
                if (type.equalsIgnoreCase(text(item, "type"))) {
                    String value = text(item, "value");
                    if (value != null) return value;
                }
            }
        }
        return fallback;
    }

    private static String firstGithubPackage(JsonNode vulnerabilities) {
        if (!vulnerabilities.isArray() || vulnerabilities.isEmpty()) return null;
        JsonNode pkg = vulnerabilities.get(0).path("package");
        String ecosystem = text(pkg, "ecosystem");
        String name = text(pkg, "name");
        if (ecosystem == null) return name;
        if (name == null) return ecosystem;
        return ecosystem + "/" + name;
    }

    private static String inferVulnType(String title, String description) {
        String text = ((title == null ? "" : title) + " " + (description == null ? "" : description)).toLowerCase();
        if (text.contains("sql injection") || text.contains("sqli")) return "SQL注入";
        if (text.contains("cross-site scripting") || text.contains("xss")) return "XSS";
        if (text.contains("remote code execution") || text.contains("code execution")) return "远程代码执行";
        if (text.contains("privilege escalation")) return "权限提升";
        if (text.contains("path traversal") || text.contains("directory traversal")) return "路径遍历";
        if (text.contains("deserialization")) return "反序列化";
        if (text.contains("authentication") || text.contains("auth bypass")) return "认证绕过";
        if (text.contains("information disclosure") || text.contains("info disclosure")) return "信息泄露";
        return "系统漏洞";
    }

    private static String normalizeSeverity(String severity) {
        if (severity == null || severity.isBlank()) return severity;
        String value = severity.trim().toUpperCase();
        return switch (value) {
            case "严重", "CRITICAL" -> "CRITICAL";
            case "高危", "HIGH" -> "HIGH";
            case "中危", "MEDIUM", "MODERATE" -> "MEDIUM";
            case "低危", "LOW" -> "LOW";
            default -> severity.trim();
        };
    }

    private static String normalizeVulnType(String vulnType) {
        if (vulnType == null || vulnType.isBlank()) return vulnType;
        String value = vulnType.trim().toLowerCase();
        if (value.equals("sql injection") || value.equals("sqli")) return "SQL注入";
        if (value.equals("remote code execution") || value.equals("rce")) return "远程代码执行";
        if (value.equals("privilege escalation")) return "权限提升";
        if (value.equals("path traversal") || value.equals("directory traversal")) return "路径遍历";
        if (value.equals("deserialization")) return "反序列化";
        if (value.equals("auth bypass") || value.equals("authentication bypass")) return "认证绕过";
        if (value.equals("information disclosure")) return "信息泄露";
        if (value.equals("system vulnerability")) return "系统漏洞";
        return vulnType.trim();
    }
}
