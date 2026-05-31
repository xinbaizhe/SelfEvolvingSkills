package com.ruoyi.team.service.impl.manifest;

import java.io.ByteArrayInputStream;
import java.util.ArrayList;
import java.util.List;
import javax.xml.parsers.DocumentBuilderFactory;
import javax.xml.xpath.XPathConstants;
import javax.xml.xpath.XPathFactory;
import org.w3c.dom.Document;
import org.w3c.dom.NodeList;

public class MavenParser implements ManifestParser {
    @Override
    public String ecosystem() { return "Maven"; }

    @Override
    public boolean supports(String fileName) {
        return fileName.toLowerCase().equals("pom.xml");
    }

    @Override
    public List<ParsedDependency> parse(String fileName, String content) {
        List<ParsedDependency> deps = new ArrayList<>();
        try {
            Document doc = DocumentBuilderFactory.newInstance()
                .newDocumentBuilder()
                .parse(new ByteArrayInputStream(content.getBytes()));
            var xpath = XPathFactory.newInstance().newXPath();
            NodeList nodes = (NodeList) xpath.evaluate(
                "//dependencies/dependency[not(scope[text()='test'])]", doc, XPathConstants.NODESET);
            for (int i = 0; i < nodes.getLength(); i++) {
                var el = nodes.item(i);
                String groupId = xpath.evaluate("groupId", el);
                String artifactId = xpath.evaluate("artifactId", el);
                String version = xpath.evaluate("version", el);
                if (artifactId != null && !artifactId.isBlank()) {
                    String name = (groupId != null ? groupId + ":" : "") + artifactId;
                    deps.add(new ParsedDependency(name, version != null ? version : ""));
                }
            }
        } catch (Exception ignored) {}
        return deps;
    }
}
