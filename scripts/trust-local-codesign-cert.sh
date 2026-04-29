#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Local code-signing certificate trust is only needed on macOS."
  exit 0
fi

IDENTITY="${WISPERGO_CODESIGN_IDENTITY:-Wispergo Local Code Signing}"
KEYCHAIN="${WISPERGO_CODESIGN_KEYCHAIN:-$(security default-keychain -d user | sed 's/^ *//; s/"//g')}"
CERT_DIR="${WISPERGO_CODESIGN_CERT_DIR:-$HOME/.wispergo}"
CERT_PATH="$CERT_DIR/wispergo-local-code-signing.cer"

mkdir -p "$CERT_DIR"

if ! security find-certificate -c "$IDENTITY" -p "$KEYCHAIN" > "$CERT_PATH"; then
  echo "Could not find certificate '$IDENTITY' in $KEYCHAIN." >&2
  echo "Run ./scripts/ensure-local-codesign-cert.sh first." >&2
  exit 1
fi

if security verify-cert -c "$CERT_PATH" -p codeSign >/dev/null 2>&1; then
  echo "Local code-signing certificate is already trusted for code signing: $IDENTITY"
  exit 0
fi

cat <<EOF
About to trust the local self-signed certificate for code signing:
  $IDENTITY

This writes trust settings to the System keychain and macOS will ask for your password.
EOF

sudo security add-trusted-cert \
  -d \
  -r trustRoot \
  -p codeSign \
  -k /Library/Keychains/System.keychain \
  "$CERT_PATH"

security verify-cert -c "$CERT_PATH" -p codeSign >/dev/null

echo "Trusted local code-signing certificate for code signing: $IDENTITY"
