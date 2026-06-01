package com.ruoyi.team.domain;

import java.time.LocalDateTime;
import java.util.List;

public record DiscoveredEndpoint(
    String url,
    String method,
    String contentType,
    boolean hasAuth,
    String discoveredFrom,
    List<String> params,
    int statusCode,
    String responsePreview,
    LocalDateTime discoveredAt
) {
    public DiscoveredEndpoint {
        if (params == null) params = List.of();
        if (responsePreview == null) responsePreview = "";
        if (discoveredAt == null) discoveredAt = LocalDateTime.now();
    }

    public String summary() {
        return method + " " + url + " [" + statusCode + "] " + contentType;
    }
}
