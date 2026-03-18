#!/bin/bash
# ── Aura Alpha Desktop: macOS Code Signing + Notarization Setup ──
# Run this on your Mac to configure GitHub secrets for signed builds.
#
# Prerequisites:
#   - Xcode installed (for codesign/security tools)
#   - gh CLI installed and authenticated (brew install gh && gh auth login)
#   - Apple Developer account with Developer ID Application cert in Keychain
#
# If you don't have a Developer ID Application cert yet:
#   1. Go to https://developer.apple.com/account/resources/certificates/add
#   2. Select "Developer ID Application"
#   3. Upload your CSR (or create one in Keychain Access → Certificate Assistant)
#   4. Download and double-click the .cer to install in Keychain
#
# Usage:
#   cd ~/AuraAlphaDesktop
#   bash scripts/setup-macos-signing.sh

set -euo pipefail
REPO="sgallup23/AuraAlphaDesktop"

echo "═══════════════════════════════════════════════"
echo "  Aura Alpha Desktop — macOS Signing Setup"
echo "═══════════════════════════════════════════════"
echo ""

# Step 1: Find Developer ID Application cert
echo "🔍 Looking for Developer ID Application certificate..."
IDENTITY=$(security find-identity -v -p codesigning | grep "Developer ID Application" | head -1 | sed 's/.*"\(.*\)"/\1/')
if [ -z "$IDENTITY" ]; then
    echo "❌ No Developer ID Application certificate found in Keychain."
    echo ""
    echo "   Create one at: https://developer.apple.com/account/resources/certificates/add"
    echo "   Select 'Developer ID Application', upload a CSR, download and install the .cer"
    echo ""
    exit 1
fi
echo "✅ Found: $IDENTITY"

# Step 2: Export as .p12
echo ""
echo "📦 Exporting certificate as .p12..."
P12_PATH="/tmp/aura_devid.p12"
P12_PASS=$(openssl rand -base64 16)
security export -t identities -f pkcs12 -k login.keychain-db -P "$P12_PASS" -o "$P12_PATH" 2>/dev/null || {
    echo "⚠️  Auto-export failed. Manually export from Keychain Access:"
    echo "   1. Open Keychain Access"
    echo "   2. Find '$IDENTITY'"
    echo "   3. Right-click → Export Items → Save as .p12"
    echo "   4. Set password and note it"
    echo ""
    read -p "Path to .p12 file: " P12_PATH
    read -s -p "Password for .p12: " P12_PASS
    echo ""
}

if [ ! -f "$P12_PATH" ]; then
    echo "❌ .p12 file not found at $P12_PATH"
    exit 1
fi

# Step 3: Base64 encode
CERT_BASE64=$(base64 -i "$P12_PATH")
echo "✅ Certificate encoded ($(echo -n "$CERT_BASE64" | wc -c | tr -d ' ') chars)"

# Step 4: Get Apple ID for notarization
echo ""
read -p "🍎 Apple ID (email) for notarization: " APPLE_ID

# Step 5: Create app-specific password
echo ""
echo "🔑 You need an app-specific password for notarization."
echo "   Create one at: https://appleid.apple.com/account/manage"
echo "   → Sign-In and Security → App-Specific Passwords → Generate"
echo "   → Name it 'Aura Alpha Notarization'"
echo ""
read -s -p "App-specific password: " APPLE_PASSWORD
echo ""

# Step 6: Set all GitHub secrets
echo ""
echo "🔐 Setting GitHub secrets on $REPO..."

gh secret set APPLE_CERTIFICATE --repo "$REPO" --body "$CERT_BASE64"
echo "  ✅ APPLE_CERTIFICATE"

gh secret set APPLE_CERTIFICATE_PASSWORD --repo "$REPO" --body "$P12_PASS"
echo "  ✅ APPLE_CERTIFICATE_PASSWORD"

gh secret set APPLE_SIGNING_IDENTITY --repo "$REPO" --body "$IDENTITY"
echo "  ✅ APPLE_SIGNING_IDENTITY"

gh secret set APPLE_ID --repo "$REPO" --body "$APPLE_ID"
echo "  ✅ APPLE_ID"

gh secret set APPLE_PASSWORD --repo "$REPO" --body "$APPLE_PASSWORD"
echo "  ✅ APPLE_PASSWORD"

# APPLE_TEAM_ID already set
echo "  ✅ APPLE_TEAM_ID (already set)"

# Cleanup
rm -f "$P12_PATH"

echo ""
echo "═══════════════════════════════════════════════"
echo "  ✅ All secrets configured!"
echo ""
echo "  To build a signed release:"
echo "    cd ~/AuraAlphaDesktop"
echo "    git tag v1.1.9"
echo "    git push origin v1.1.9"
echo ""
echo "  The GitHub Action will build, sign, notarize,"
echo "  and upload to a GitHub Release automatically."
echo "═══════════════════════════════════════════════"
