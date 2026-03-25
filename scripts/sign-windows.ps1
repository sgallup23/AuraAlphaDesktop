# Aura Alpha — Windows Code Signing Script
# Signs all build artifacts (.exe, .msi) after Tauri build.
#
# Usage:
#   .\scripts\sign-windows.ps1                      # Standard PFX cert
#   .\scripts\sign-windows.ps1 -EV                  # EV token/HSM signing
#
# Environment Variables (Standard):
#   SIGN_CERT_PATH       - Path to .pfx file
#   SIGN_CERT_PASSWORD   - PFX password
#
# Environment Variables (EV):
#   SIGN_CERT_NAME       - Certificate subject name
#   SIGN_CERT_PROVIDER   - CSP provider (e.g., "eToken Base Cryptographic Provider")
#
# Common:
#   TIMESTAMP_URL        - Timestamp server (default: http://ts.ssl.com)

param(
    [switch]$EV,
    [string]$ArtifactPath = "src-tauri\target\release\bundle"
)

$ErrorActionPreference = "Stop"
$TimestampUrl = $env:TIMESTAMP_URL ?? "http://ts.ssl.com"

Write-Host "================================================" -ForegroundColor Cyan
Write-Host "  Aura Alpha — Windows Code Signing" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

# ── Detect signing method ────────────────────────────────────────────

if ($EV -or $env:SIGN_CERT_PROVIDER) {
    $SignMethod = "EV"
    $CertName = $env:SIGN_CERT_NAME
    $CertProvider = $env:SIGN_CERT_PROVIDER ?? "eToken Base Cryptographic Provider"

    if (-not $CertName) {
        Write-Host "ERROR: SIGN_CERT_NAME not set for EV signing" -ForegroundColor Red
        exit 1
    }
    Write-Host "Signing method: EV Token (CSP: $CertProvider)" -ForegroundColor Yellow
} elseif ($env:SIGN_CERT_PATH) {
    $SignMethod = "Standard"
    $CertPath = $env:SIGN_CERT_PATH
    $CertPassword = $env:SIGN_CERT_PASSWORD

    if (-not (Test-Path $CertPath)) {
        Write-Host "ERROR: Certificate not found at $CertPath" -ForegroundColor Red
        exit 1
    }
    if (-not $CertPassword) {
        Write-Host "ERROR: SIGN_CERT_PASSWORD not set" -ForegroundColor Red
        exit 1
    }
    Write-Host "Signing method: Standard PFX ($CertPath)" -ForegroundColor Yellow
} else {
    Write-Host "WARNING: No signing credentials found. Skipping signing." -ForegroundColor Yellow
    Write-Host "Set SIGN_CERT_PATH + SIGN_CERT_PASSWORD (standard) or SIGN_CERT_NAME + SIGN_CERT_PROVIDER (EV)" -ForegroundColor Gray
    exit 0
}

Write-Host "Timestamp: $TimestampUrl" -ForegroundColor Gray
Write-Host ""

# ── Find all artifacts to sign ───────────────────────────────────────

$artifacts = Get-ChildItem -Path $ArtifactPath -Recurse -Include "*.exe", "*.msi" -ErrorAction SilentlyContinue
if ($artifacts.Count -eq 0) {
    Write-Host "ERROR: No .exe or .msi files found in $ArtifactPath" -ForegroundColor Red
    exit 1
}

Write-Host "Found $($artifacts.Count) artifacts to sign:" -ForegroundColor Green
$artifacts | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }
Write-Host ""

# ── Sign each artifact ───────────────────────────────────────────────

$signed = 0
$failed = 0

foreach ($file in $artifacts) {
    Write-Host "Signing: $($file.Name)..." -NoNewline

    try {
        if ($SignMethod -eq "EV") {
            & signtool sign /tr $TimestampUrl /td sha256 /fd sha256 `
                /n "$CertName" /csp "$CertProvider" $file.FullName
        } else {
            & signtool sign /f $CertPath /p $CertPassword `
                /tr $TimestampUrl /td sha256 /fd sha256 $file.FullName
        }

        if ($LASTEXITCODE -eq 0) {
            Write-Host " OK" -ForegroundColor Green
            $signed++
        } else {
            Write-Host " FAILED (exit code $LASTEXITCODE)" -ForegroundColor Red
            $failed++
        }
    } catch {
        Write-Host " ERROR: $_" -ForegroundColor Red
        $failed++
    }
}

Write-Host ""

# ── Verify signatures ────────────────────────────────────────────────

Write-Host "Verifying signatures..." -ForegroundColor Cyan
$verified = 0

foreach ($file in $artifacts) {
    $result = & signtool verify /pa /tw $file.FullName 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  VERIFIED: $($file.Name)" -ForegroundColor Green
        $verified++
    } else {
        Write-Host "  UNVERIFIED: $($file.Name)" -ForegroundColor Red
    }
}

Write-Host ""
Write-Host "================================================" -ForegroundColor Cyan
Write-Host "  Results: $signed signed, $verified verified, $failed failed" -ForegroundColor $(if ($failed -gt 0) { "Red" } else { "Green" })
Write-Host "================================================" -ForegroundColor Cyan

if ($failed -gt 0) {
    Write-Host "BUILD FAILED: $failed artifacts could not be signed" -ForegroundColor Red
    exit 1
}
