# Abuse response and moderation

No upload service can prove that every submission is benign. copypaste.fyi combines admission
control, short retention, quarantine, targeted moderation, provider reporting, and human response.
This runbook is operational guidance, not legal advice. Have qualified counsel adapt it to every
jurisdiction in which the service or its users operate.

## Non-negotiable safety rule

Do not open, decrypt, render, download, screenshot, copy, forward, classify, or otherwise inspect a
reported paste merely to validate a complaint. Do not ask the reporter to resend the material.

Use the report's exact URL or ID only as a routing value in the restricted incident record,
quarantine configuration, and targeted admin request. Do not search nearby IDs, browse unrelated
accounts, or create a local collection of suspected illegal material.

If a person may be in immediate danger, contact the appropriate emergency service instead of
waiting for the normal moderation queue.

## Prepare before accepting writes

Complete these controls before exposing a deployment to untrusted users:

1. Set `COPYPASTE_REQUIRE_WRITE_AUTH=true` and require a service credential in
   `X-CopyPaste-Write-Token`. A signed session in `Authorization` may supply identity, but it is not
   admission under the supplied Fly policy. Static write and admin tokens must contain 43 to 128
   base64url characters (`A-Z`, `a-z`, `0-9`, `_`, and `-`); invalid non-empty values stop startup.
2. Put writes behind shared edge quotas, request-size limits, and bot controls. Application rate
   limits are process-local backstops.
3. Run one app instance. Sessions, dashboards, cached indexes, burn behavior, and mutation ordering
   are not distributed.
4. Give untrusted content short retention and a small size limit. Keep bundles, attestations,
   webhooks, and steganography disabled.
5. Configure a monitored abuse channel that accepts a URL or ID without an attachment.
6. Suppress or redact admin-route path parameters in Rocket request logs, edge logs, and reverse
   proxy logs. The custom moderation audit event contains no target identifier, but ordinary request
   logs can otherwise record the URL path.
7. Alert on write bursts, repeated moderation outcomes, storage errors, and provider complaints.
   Logs must exclude paste bodies, ciphertext, keys, workspace labels, attestation material,
   webhook destinations, and reported IDs.
8. Maintain an authoritative, access-controlled quarantine list outside Fly. Fly secrets are not a
   retrievable source of truth.

Do not build a homemade illegal-content hash collection. Use an appropriate provider program only
after legal and safety review.

## Establish administrator access

The current Fly deployment does not configure `COPYPASTE_SQLITE_PATH`. Dynamic API-key management
therefore returns `503`, and moderation depends on `COPYPASTE_ADMIN_TOKEN`.

Before importing that token into Fly:

1. Use an approved password manager to generate at least 256 bits of random material encoded as 43
   to 128 base64url characters (`A-Z`, `a-z`, `0-9`, `_`, and `-`).
2. Save it as the deployment's current `COPYPASTE_ADMIN_TOKEN` record.
3. Confirm that authorized responders can retrieve it and that the password-manager audit policy is
   active.
4. Import that already-stored value into Fly. Never make Fly the only copy; Fly does not reveal a
   secret after import.

Retrieve the saved token into a temporary shell variable without placing it in command history:

```bash
(
set -eu
set +x

printf 'Admin token from password manager: ' >&2
IFS= read -r -s copypaste_admin_token
printf '\n' >&2

case "${copypaste_admin_token}" in
  *[!A-Za-z0-9_-]*|'')
    printf 'Admin token must use base64url characters only.\n' >&2
    exit 1
    ;;
esac
if [ "${#copypaste_admin_token}" -lt 43 ] || [ "${#copypaste_admin_token}" -gt 128 ]; then
  printf 'Admin token must contain 43 to 128 base64url characters.\n' >&2
  exit 1
fi

printf 'COPYPASTE_ADMIN_TOKEN=%s\n' "${copypaste_admin_token}" | fly secrets import
unset copypaste_admin_token
)
```

Unsetting the temporary shell variable is not deleting the credential. Keep the current credential
in the approved password manager for routine access and rotation. Never discard the only
recoverable copy. When rotating, save the replacement first, import it, verify every app instance
has restarted with it, and retain rotation history according to organizational policy.

Restrict admin routes at the edge with an operator VPN or identity-aware proxy in addition to
application authentication.

Dynamic admin keys are available only when an operator explicitly configures a safe
`COPYPASTE_SQLITE_PATH`. On Unix, its existing parent directory must belong to the service user and
be mode `0700`; the database is owner-only mode `0600`. An unsafe explicit path stops startup. On
other platforms, apply equivalent owner-only ACLs because the app does not verify Unix ownership
or modes there. A local SQLite file is not shared across Fly VMs, so it does not remove the
single-app-instance limit.

## Understand the moderation API

`GET /api/admin/pastes/{id}` returns bounded metadata only:

- `id`
- `format`
- `createdAt` and optional `expiresAt`
- `burnAfterReading`
- `encrypted` and `encryptionAlgorithm`
- `approximateStoredBytes`
- `torAccessOnly`
- Boolean `hasAttestation`, `hasWebhook`, and `hasWorkspace` flags

The response has no `accessCount` field and no workspace label. It never returns paste text,
ciphertext, keys, nonces, salts, ownership tokens, attestation secrets, or webhook configuration.
Administrators have no master decryption key. There is no bulk paste-listing or content-preview
endpoint.

`DELETE /api/admin/pastes/{id}` targets one exact ID and does not load its body before the delete.
A storage failure returns `503`; it is not reported as a completed takedown.

The application moderation audit event records only:

- Administrator key ID
- Action (`inspect` or `delete`)
- Outcome

It records no raw paste ID, target digest, content, or access count. Keep the report-to-action
mapping only in the restricted incident system. This guarantee applies to the custom moderation
event, not ordinary Rocket, edge, or proxy request logging. Suppress or redact the admin-route path
in every request log before using these endpoints.

## Quarantine before triage or deletion

Quarantine is the first application action. `COPYPASTE_BLOCKED_PASTE_IDS` is a comma-separated exact
set. Public JSON, share, legacy HTML, raw, update, finalize, and anchor routes return `404` for a
blocked ID before public storage access. The metadata-only admin route remains available.

The environment value is deployment-time state, not a live distributed database. Every app
instance must restart with the authoritative full set. Updating a Fly secret replaces its prior
value, and Fly does not return the plaintext. Importing only the newest report would silently
release every older quarantine.

Maintain one newline-delimited authoritative file in the restricted incident system. Add the new ID
there, have a second responder verify that all prior entries remain, and import the complete set:

```bash
printf 'Path to authoritative quarantine file: ' >&2
IFS= read -r copypaste_quarantine_file
test -r "${copypaste_quarantine_file}"
awk 'NF && $0 !~ /^[A-Za-z0-9_-]{1,128}$/ { exit 1 }' "${copypaste_quarantine_file}"
copypaste_quarantine_ids="$(awk 'NF { printf "%s%s", separator, $0; separator="," }' "${copypaste_quarantine_file}")"
test -n "${copypaste_quarantine_ids}"
printf 'COPYPASTE_BLOCKED_PASTE_IDS=%s\n' "${copypaste_quarantine_ids}" | fly secrets import
unset copypaste_quarantine_ids copypaste_quarantine_file
```

After import, use the Fly machine inventory and release/config version to verify that every `app`
instance has been replaced or restarted with the new secret. Account for stopped and automatically
started machines, not only the instance currently serving traffic. Do not verify quarantine by
requesting or opening the reported paste.

Do not continue to deletion until every app instance is on the quarantined configuration. A stale
instance can retain cached data or complete an older live-paste save.

## Triage metadata without content

After quarantine and full rollout, retrieve the admin token and reported ID through hidden prompts.
Validate both values before constructing the request. Put the authorization header in a temporary
mode-`0600` file so the admin token never appears in curl's argv. This example requires `curl`,
`jq`, and `mktemp`:

```bash
(
set -eu
set -o pipefail
set +x

printf 'Admin token from password manager: ' >&2
IFS= read -r -s copypaste_admin_token
printf '\nReported paste ID from restricted incident record: ' >&2
IFS= read -r -s reported_paste_id
printf '\n' >&2

case "${copypaste_admin_token}" in
  *[!A-Za-z0-9_-]*|'')
    printf 'Admin token must use base64url characters only.\n' >&2
    exit 1
    ;;
esac
if [ "${#copypaste_admin_token}" -lt 43 ] || [ "${#copypaste_admin_token}" -gt 128 ]; then
  printf 'Admin token must contain 43 to 128 base64url characters.\n' >&2
  exit 1
fi
case "${reported_paste_id}" in
  *[!A-Za-z0-9_-]*|'')
    printf 'Paste ID must use URL-safe identifier characters only.\n' >&2
    exit 1
    ;;
esac
if [ "${#reported_paste_id}" -gt 128 ]; then
  printf 'Paste ID must contain 1 to 128 characters.\n' >&2
  exit 1
fi

umask 077
copypaste_header_file="$(mktemp)"
chmod 600 "${copypaste_header_file}"
copypaste_cleanup() {
  rm -f -- "${copypaste_header_file}"
}
trap copypaste_cleanup EXIT
trap 'exit 130' HUP INT TERM

printf 'Authorization: Bearer %s\n' "${copypaste_admin_token}" > "${copypaste_header_file}"
unset copypaste_admin_token

curl --fail-with-body --silent --show-error \
  --header @"${copypaste_header_file}" \
  -- "https://api.example.test/api/admin/pastes/${reported_paste_id}" \
  | jq 'del(.id)'

unset reported_paste_id
)
```

Record only the fields needed for the decision. Do not fetch the public, share, raw, or JSON content
route. Do not supply an encryption key.

## Decide preservation and reporting

Escalate the notice to the designated safety lead and qualified counsel. Determine whether the
applicable law or an authorized agency requires preservation before deletion. The ordinary paste
store and application logs are not evidence systems. If preservation is required, keep public
access quarantined and follow counsel-approved evidence procedures with strict access controls.

For suspected child sexual abuse material, use the applicable electronic-service-provider process
and follow instructions from the National Center for Missing & Exploited Children (NCMEC) or law
enforcement. NCMEC describes the CyberTipline as the United States' centralized reporting system for
online child exploitation and accepts reports from the public and electronic service providers:
[NCMEC CyberTipline](https://www.missingkids.org/gethelpnow/cybertipline).

Do not attach suspected material to a provider, registrar, or incident-management reply. Respond
with the external case number, the time access was quarantined, and the reporting/escalation status.

## Delete only after the rollout gate

Delete only when all of these conditions are true:

- The ID is in the authoritative full quarantine list.
- Every app instance has restarted with that full list.
- The preservation decision authorizes deletion.
- The responder has the retained administrator credential.

Use the same validation and temporary owner-only header file for deletion. The token is never placed
in curl's argv:

```bash
(
set -eu
set +x

printf 'Admin token from password manager: ' >&2
IFS= read -r -s copypaste_admin_token
printf '\nReported paste ID from restricted incident record: ' >&2
IFS= read -r -s reported_paste_id
printf '\n' >&2

case "${copypaste_admin_token}" in
  *[!A-Za-z0-9_-]*|'')
    printf 'Admin token must use base64url characters only.\n' >&2
    exit 1
    ;;
esac
if [ "${#copypaste_admin_token}" -lt 43 ] || [ "${#copypaste_admin_token}" -gt 128 ]; then
  printf 'Admin token must contain 43 to 128 base64url characters.\n' >&2
  exit 1
fi
case "${reported_paste_id}" in
  *[!A-Za-z0-9_-]*|'')
    printf 'Paste ID must use URL-safe identifier characters only.\n' >&2
    exit 1
    ;;
esac
if [ "${#reported_paste_id}" -gt 128 ]; then
  printf 'Paste ID must contain 1 to 128 characters.\n' >&2
  exit 1
fi

umask 077
copypaste_header_file="$(mktemp)"
chmod 600 "${copypaste_header_file}"
copypaste_cleanup() {
  rm -f -- "${copypaste_header_file}"
}
trap copypaste_cleanup EXIT
trap 'exit 130' HUP INT TERM

printf 'Authorization: Bearer %s\n' "${copypaste_admin_token}" > "${copypaste_header_file}"
unset copypaste_admin_token

curl --request DELETE --fail-with-body --silent --show-error \
  --output /dev/null \
  --header @"${copypaste_header_file}" \
  -- "https://api.example.test/api/admin/pastes/${reported_paste_id}"

unset reported_paste_id
)
```

A successful response confirms only that the handling app instance completed its configured
backing-store delete request and cleared its local cache. It does not prove cross-instance absence.
Another instance could have held a stale cache or in-flight save before the rollout, and the Redis
adapter has no distributed version or tombstone to prevent resurrection.

Keep the ID on the authoritative quarantine list after deletion. Remove it only through a separate,
reviewed change after the organization has established that no stale instance or recovery path can
restore public access. A `503` means deletion was not confirmed; keep quarantine in place, record the
failure, investigate storage health, and retry only through the controlled incident process.

## Close the incident

1. Record quarantine rollout evidence, metadata triage, the preservation decision, delete outcome,
   external case numbers, and timestamps in the restricted incident record.
2. Verify that application moderation audit events contain only admin key ID, action, and outcome.
   Treat any target identifier in logs as a logging incident and remove it under retention policy.
3. Rotate credentials used outside their normal handling boundary. Save every replacement in the
   approved password manager before importing it, then verify full app rollout. Do not discard the
   current administrator credential.
4. Review write-admission, edge-rate, retention, and alerting controls. Add regression tests for the
   path that failed.
5. Keep the authoritative quarantine list complete. Never rebuild it from Fly secrets or memory.

This procedure limits operator exposure and public access. It cannot establish that content never
existed, that no user copied it before quarantine, or that deletion removed every external copy.
