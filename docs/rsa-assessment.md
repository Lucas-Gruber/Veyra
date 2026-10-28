# RSA dependency assessment — 2026-10-28

This records a source review of Veyra and the official Bitwarden SDK pinned at
`4988da44b0ac55778e3a45ec7363394509298866`. It is not an independent security
certification, a formal proof, or an advisory exception. The release audit stays
enabled and failing for **RUSTSEC-2023-0071** until its finding is resolved.

## Finding and reviewed paths

[RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html)
describes recovery of RSA private keys through timing observations of private-key
decryption. RustSec currently lists no patched version. Updating the pinned SDK
to current upstream does not remove its mandatory `rsa` dependency.

In this SDK, the reviewed vulnerable sink is
`UnsignedSharedKey::decapsulate_key_unsigned` in
`crates/bitwarden-crypto/src/enc_string/asymmetric.rs`, which invokes
`rsa_private_key.decrypt(Oaep<…>, data)`. Veyra does not call the organization,
sharing, auth-request or device-key APIs that use this operation.

Veyra accepts V1 personal vaults. Account key envelopes must be authenticated AES
format (type 2); signed/V2 account states are rejected. Organization ciphers are
excluded before conversion to SDK ciphers, and organization keys from the sync
profile are never initialized. Folder, item-key and field decryption use SDK
`EncString` and symmetric key slots. RSA envelopes (types 3–6) are a separate SDK
type; they are not a fallback for personal symmetric decryption.

The previous SDK V1 initializer also unwrapped and imported the account's unused
RSA private key. Personal-only initialization now uses the SDK's unchanged
`context_mut`, `add_local_symmetric_key` and `persist_symmetric_key` APIs to retain
only the AES user key. The encrypted `privateKey` envelope is still checked for
its supported format, but its decrypted RSA contents are not unwrapped, parsed,
authenticated or imported. This does not claim verification of an unused account
RSA key. AES-key type, fresh-store guards and strict cipher verification remain
required. Locking still clears the entire SDK key store.

The SDK may process additional subpayloads inside a supported cipher, including
an SSH key subpayload. The reviewed import path parses an SSH/RSA key and derives
public metadata; it does not invoke the vulnerable OAEP decryption sink. This
assessment does not claim that Veyra never handles any RSA-related data.

## Regression checks and limits

The live PBKDF2/Argon2 and authenticator-login tests check for a usable personal
AES key and the absence of account RSA/signing keys. Hostile sync responses with
RSA envelopes in personal item names and keys must fail and preserve the previous
complete snapshot. These cases supplement actual successful login, decryption,
sync, 2FA and lock checks; skipped integration tests are not passing evidence.

This is manual source review with targeted regression checks, not exhaustive
instrumented call coverage. Any SDK update or new organization, sharing,
device-login, auth-request, SSH or account-crypto support requires a fresh review.
The dependency remains in the graph, so this hardening alone does not make
`cargo audit` pass or authorize publication.
