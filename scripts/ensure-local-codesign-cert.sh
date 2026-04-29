#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Skipping local code-signing certificate setup on non-Darwin host."
  exit 0
fi

IDENTITY="${WISPERGO_CODESIGN_IDENTITY:-Wispergo Local Code Signing}"
KEYCHAIN="${WISPERGO_CODESIGN_KEYCHAIN:-$(security default-keychain -d user | sed 's/^ *//; s/"//g')}"
P12_PASSWORD="${WISPERGO_CODESIGN_P12_PASSWORD:-wispergo-local}"
OPENSSL_BIN="${OPENSSL_BIN:-openssl}"

can_codesign_with_identity() {
  local tmp_dir test_binary
  tmp_dir="$(mktemp -d)"
  test_binary="$tmp_dir/echo"
  cp /bin/echo "$test_binary"
  if codesign --force --sign "$IDENTITY" --keychain "$KEYCHAIN" "$test_binary" >/dev/null 2>&1; then
    rm -rf "$tmp_dir"
    return 0
  fi
  rm -rf "$tmp_dir"
  return 1
}

if can_codesign_with_identity; then
  echo "Using existing local code-signing identity: $IDENTITY"
  exit 0
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

CERT_PEM="$TMP_DIR/codesign.crt"
KEY_PEM="$TMP_DIR/codesign.key"
P12="$TMP_DIR/codesign.p12"

"$OPENSSL_BIN" req \
  -x509 \
  -newkey rsa:2048 \
  -sha256 \
  -days 3650 \
  -nodes \
  -subj "/CN=$IDENTITY/" \
  -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=codeSigning" \
  -keyout "$KEY_PEM" \
  -out "$CERT_PEM" >/dev/null 2>&1

"$OPENSSL_BIN" pkcs12 \
  -export \
  -legacy \
  -name "$IDENTITY" \
  -inkey "$KEY_PEM" \
  -in "$CERT_PEM" \
  -out "$P12" \
  -passout "pass:$P12_PASSWORD" >/dev/null 2>&1

security import "$P12" \
  -k "$KEYCHAIN" \
  -P "$P12_PASSWORD" \
  -T /usr/bin/codesign \
  -T /usr/bin/security >/dev/null

if [[ "${WISPERGO_TRUST_LOCAL_CERT:-0}" == "1" ]]; then
  security add-trusted-cert \
    -r trustRoot \
    -p codeSign \
    -k "$KEYCHAIN" \
    "$CERT_PEM" >/dev/null
fi

if ! can_codesign_with_identity; then
  echo "Created certificate but codesign cannot use it: $IDENTITY" >&2
  exit 1
fi

echo "Created local code-signing identity: $IDENTITY"
