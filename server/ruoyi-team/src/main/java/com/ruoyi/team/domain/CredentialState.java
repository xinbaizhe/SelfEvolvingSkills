package com.ruoyi.team.domain;

public record CredentialState(
    String credId,
    String role,
    String username,
    String permissions,
    boolean sessionValid,
    String cookie,
    String authorization
) {
    public String label() {
        return role + " (" + username + ")" + (sessionValid ? "" : " [expired]");
    }
}
