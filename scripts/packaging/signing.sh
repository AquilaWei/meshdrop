#!/usr/bin/env bash
# Source this file to export the signing path in the caller. Never enable xtrace.
set +x
set -euo pipefail
signing_directory="$(mktemp -d /tmp/meshdrop-signing-XXXXXX)"
export MESHDROP_KEYSTORE_PATH="$signing_directory/release.jks"
trap 'rm -rf "$signing_directory"' EXIT
if [[ "${GITHUB_EVENT_NAME:-}" == workflow_dispatch ]]; then
  export MESHDROP_KEYSTORE_PASSWORD="$(openssl rand -hex 24)"
  export MESHDROP_KEY_PASSWORD="$MESHDROP_KEYSTORE_PASSWORD"
  export MESHDROP_KEY_ALIAS=ephemeral-dry-run
  if [[ "${GITHUB_ACTIONS:-}" == true ]]; then
    echo "::add-mask::$MESHDROP_KEYSTORE_PASSWORD"
  fi
  keytool -genkeypair -keystore "$MESHDROP_KEYSTORE_PATH" \
    -storepass:env MESHDROP_KEYSTORE_PASSWORD -keypass:env MESHDROP_KEY_PASSWORD \
    -alias "$MESHDROP_KEY_ALIAS" -keyalg RSA -keysize 3072 -validity 2 \
    -dname 'CN=meshdrop dry run NOT FOR DISTRIBUTION' >/dev/null 2>&1
else
  : "${MESHDROP_KEYSTORE_BASE64:?Tag builds require the release keystore secret}"
  : "${MESHDROP_KEYSTORE_PASSWORD:?Missing keystore password}"
  : "${MESHDROP_KEY_ALIAS:?Missing signing alias}"
  : "${MESHDROP_KEY_PASSWORD:?Missing signing key password}"
  printf '%s' "$MESHDROP_KEYSTORE_BASE64" | base64 --decode > "$MESHDROP_KEYSTORE_PATH"
fi
chmod 600 "$MESHDROP_KEYSTORE_PATH"
