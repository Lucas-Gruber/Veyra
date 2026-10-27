# Vaultwarden versions and updates

Veyra implements a tested subset of the Bitwarden-compatible password-vault API. A Vaultwarden update does not inherently require a Veyra update, but compatibility is demonstrated by tests, not assumed from the version number.

## Supported boundary

The initial reference server is Vaultwarden **1.37.4**, pinned to its official image digest. The connection uses personal-vault read-only synchronization, bounded PBKDF2/Argon2id parameters and official Bitwarden cryptographic components pinned to a specific source commit. Organization keys, newer item/encryption types, authentication methods and account migrations outside this subset must be reported explicitly. No real data may silently be replaced with demonstration data.

The API may add response fields; parsers can ignore unrelated fields while requiring the fields needed to authenticate/decrypt. Unknown KDFs, excessive work parameters, unsupported cryptographic states, failed integrity checks and malformed required data fail with safe errors. Veyra must never compensate by lowering KDF settings or disabling TLS or integrity verification. A failed refresh preserves the prior complete snapshot; it does not report a successful sync.

## Update detection

`.github/workflows/vaultwarden-compatibility.yml` tests the pinned baseline and resolves the latest **stable** upstream GitHub release to an immutable container digest. It runs daily, manually and as part of CI/release validation. When latest matches both the baseline version and image digest, one job covers both. Retagged images are tested separately. Each job creates fresh synthetic PBKDF2 and Argon2 accounts in a disposable local server, then exercises real authentication, field decryption, sync, wrong-password rejection and locking.

A failing matrix blocks the release workflow. It can indicate either a client regression, an upstream API/crypto change, or a test-fixture registration change; inspect the failing step before assigning the cause. A workflow file alone does not establish that the remote scheduled job has run. `docs/validation.md` records only actual executions.

Veyra does not update, administer or change your Vaultwarden server. Keep the server maintained and backed up. For a protocol or encryption change, the client needs an adapted and tested release before that new behavior is supported. There is no guarantee for arbitrary future versions, nightly images or every deployment configuration.

## Reproducing a failure

Use the isolated fixture server and documented integration test command in `tests/integration/README.md`. Never use a production vault or put real credentials in CI. Record the exact server image digest, SDK source commit, Veyra commit and failed behavior. Update pins deliberately after the baseline/new-version matrix passes; do not automatically loosen compatibility checks when the upstream version changes.
