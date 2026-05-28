package com.ruoyi.team.domain;

public class VulnFinding {
    private Long id;
    private Long jobId;
    private String severity;
    private String type;
    private String location;
    private String description;
    private String suggestion;

    public VulnFinding() {}

    public VulnFinding(Long jobId, String severity, String type, String location, String description, String suggestion) {
        this.jobId = jobId;
        this.severity = severity;
        this.type = type;
        this.location = location;
        this.description = description;
        this.suggestion = suggestion;
    }

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }
    public Long getJobId() { return jobId; }
    public void setJobId(Long jobId) { this.jobId = jobId; }
    public String getSeverity() { return severity; }
    public void setSeverity(String severity) { this.severity = severity; }
    public String getType() { return type; }
    public void setType(String type) { this.type = type; }
    public String getLocation() { return location; }
    public void setLocation(String location) { this.location = location; }
    public String getDescription() { return description; }
    public void setDescription(String description) { this.description = description; }
    public String getSuggestion() { return suggestion; }
    public void setSuggestion(String suggestion) { this.suggestion = suggestion; }
}
