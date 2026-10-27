#!/usr/bin/env python3
"""Independent Bitwarden V1 protocol fixtures for a dedicated loopback test server.

TEST ONLY, never production application crypto. Reference: official sdk-internal
keys/kdf.rs, master_key.rs, utils.rs, rsa.rs and enc_string/symmetric.rs.
Before server operations, assert the SDK's public PBKDF2/Argon2/HKDF test vectors.
Secrets stay in memory or a 0600 fixture file outside the repository; no tokens
or passwords are printed or persisted in server-request logs by this helper.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import ipaddress
import hmac
import json
import os
import secrets
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from pathlib import Path

from argon2.low_level import Type, hash_secret_raw
from cryptography.hazmat.primitives import hashes, padding, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.primitives.kdf.hkdf import HKDFExpand

SERVER = ''
OUTPUT = Path()


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, file_pointer, code, message, headers, new_url):
        return None


# Only dedicated literal-loopback test servers are supported. Never forward a
# bearer token or test password hash through a redirect or to a remote origin.
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())


class FixtureError(Exception):
    pass


def b64(data: bytes) -> str:
    return base64.b64encode(data).decode('ascii')


def derive_master(password: str, email: str, kdf: dict) -> bytes:
    password_bytes = password.encode('utf-8')
    salt = email.strip().lower().encode('utf-8')
    if kdf['kdf'] == 0:
        return hashlib.pbkdf2_hmac('sha256', password_bytes, salt, kdf['kdfIterations'], 32)
    if kdf['kdf'] != 1:
        raise FixtureError('Unsupported test KDF')
    return hash_secret_raw(
        secret=password_bytes, salt=hashlib.sha256(salt).digest(),
        time_cost=kdf['kdfIterations'], memory_cost=kdf['kdfMemory'] * 1024,
        parallelism=kdf['kdfParallelism'], hash_len=32, type=Type.ID, version=19,
    )


def password_hash(master: bytes, password: str) -> str:
    return b64(hashlib.pbkdf2_hmac('sha256', master, password.encode('utf-8'), 1, 32))


def stretch(master: bytes) -> bytes:
    return b''.join(HKDFExpand(algorithm=hashes.SHA256(), length=32, info=info).derive(master)
                    for info in (b'enc', b'mac'))


def encrypt(data: bytes | str, key: bytes) -> str:
    if isinstance(data, str):
        data = data.encode('utf-8')
    if len(key) != 64:
        raise FixtureError('A V1 fixture key must have 64 bytes')
    iv = secrets.token_bytes(16)
    padder = padding.PKCS7(128).padder()
    padded = padder.update(data) + padder.finalize()
    enc = Cipher(algorithms.AES(key[:32]), modes.CBC(iv)).encryptor()
    ciphertext = enc.update(padded) + enc.finalize()
    mac = hmac.new(key[32:], iv + ciphertext, hashlib.sha256).digest()
    return '2.' + '|'.join(map(b64, (iv, ciphertext, mac)))


def decrypt(value: str, key: bytes) -> bytes:
    prefix, encoded = value.split('.', 1)
    if prefix != '2':
        raise FixtureError('Unexpected fixture ciphertext type')
    iv, ciphertext, mac = map(base64.b64decode, encoded.split('|'))
    expected = hmac.new(key[32:], iv + ciphertext, hashlib.sha256).digest()
    if not hmac.compare_digest(mac, expected):
        raise FixtureError('Fixture ciphertext MAC verification failed')
    dec = Cipher(algorithms.AES(key[:32]), modes.CBC(iv)).decryptor()
    padded = dec.update(ciphertext) + dec.finalize()
    unpadder = padding.PKCS7(128).unpadder()
    return unpadder.update(padded) + unpadder.finalize()


def check_sdk_vectors() -> None:
    pbkdf = {'kdf': 0, 'kdfIterations': 100000}
    assert password_hash(derive_master('asdfasdf', ' TEST@bitwarden.com', pbkdf), 'asdfasdf') == 'wmyadRMyBZOH7P/a/ucTCbSghKgdzDpPqUnu/DAVtSw='
    argon = {'kdf': 1, 'kdfIterations': 4, 'kdfMemory': 32, 'kdfParallelism': 2}
    assert password_hash(derive_master('asdfasdf', 'test_salt', argon), 'asdfasdf') == 'PR6UjYmjmppTYcdyTiNbAhPJuQQOmynKbdEl1oyi/iQ='
    master = derive_master('67t9b5g67$%Dh89n', 'test_key', {'kdf': 0, 'kdfIterations': 10000})
    assert master == bytes([31,79,104,226,150,71,177,90,194,80,172,209,17,129,132,81,138,167,69,167,254,149,2,27,39,197,64,42,22,195,86,75])
    stretched = stretch(master)
    assert stretched[:32] == bytes([111,31,178,45,238,152,37,114,143,215,124,83,135,173,195,23,142,134,120,249,61,132,163,182,113,197,189,204,188,21,237,96])
    assert stretched[32:] == bytes([221,127,206,234,101,27,202,38,86,52,34,28,78,28,185,16,48,61,127,166,209,247,194,87,232,26,48,85,193,249,179,155])
    assert totp('GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ', 59, 8) == '94287082'


def request(path: str, *, data: dict | None = None, form: dict | None = None,
            token: str | None = None, allow_error: bool = False,
            error_json: bool = False) -> tuple[int, dict | str | None]:
    # Fixed literal loopback origin; fixture HTTP can never be enabled for remote hosts.
    headers = {'Accept': 'application/json', 'Device-Type': '8'}
    payload = None
    if data is not None:
        payload = json.dumps(data).encode('utf-8')
        headers['Content-Type'] = 'application/json'
    if form is not None:
        payload = urllib.parse.urlencode(form).encode('utf-8')
        headers['Content-Type'] = 'application/x-www-form-urlencoded'
    if token:
        headers['Authorization'] = 'Bearer ' + token
    req = urllib.request.Request(SERVER + path, data=payload, headers=headers)
    try:
        with HTTP.open(req, timeout=30) as response:
            raw = response.read(16 * 1024 * 1024 + 1)
            if len(raw) > 16 * 1024 * 1024:
                raise FixtureError('Fixture response exceeded its size limit')
            return response.status, json.loads(raw) if raw else None
    except urllib.error.HTTPError as exc:
        if allow_error:
            if error_json:
                raw = exc.read(256 * 1024 + 1)
                if len(raw) > 256 * 1024:
                    raise FixtureError('Fixture identity error exceeded its size limit')
                return exc.code, json.loads(raw) if raw else None
            return exc.code, None
        # Never include request or response bodies in errors: they may contain test secrets.
        raise FixtureError(f'Fixture request {path.split("?")[0]} returned HTTP {exc.code}') from None


def login(account: dict, *, wrong: bool = False, two_factor: str | None = None,
          expected_error: bool = False) -> tuple[int, dict | None]:
    master = derive_master(account['password'], account['email'], account['kdf'])
    phash = password_hash(master, account['password'])
    form = {
        'grant_type': 'password', 'username': account['email'],
        'password': b64(secrets.token_bytes(32)) if wrong else phash,
        'scope': 'api offline_access', 'client_id': 'desktop',
        'deviceIdentifier': str(uuid.uuid4()), 'deviceName': 'Veyra integration fixture', 'deviceType': '8',
    }
    if two_factor is not None:
        form.update({'twoFactorProvider': '0', 'twoFactorToken': two_factor, 'twoFactorRemember': '0'})
    return request('/identity/connect/token', form=form, allow_error=wrong or expected_error, error_json=expected_error)


def totp(seed: str, timestamp: float | None = None, digits: int = 6) -> str:
    """RFC 6238 SHA-1 reference used only to exercise the real server flow."""
    timestamp = time.time() if timestamp is None else timestamp
    counter = int(timestamp // 30).to_bytes(8, 'big')
    digest = hmac.new(base64.b32decode(seed, casefold=True), counter, hashlib.sha1).digest()
    offset = digest[-1] & 0x0f
    value = int.from_bytes(digest[offset:offset + 4], 'big') & 0x7fffffff
    return str(value % (10 ** digits)).zfill(digits)


def wait_next_totp_step() -> None:
    # Vaultwarden rejects reuse of a successful TOTP time step. One bounded wait
    # uses a genuinely new code; it does not bypass or disable replay protection.
    time.sleep(30 - time.time() % 30 + 0.1)


def enable_authenticator(account: dict) -> None:
    _, authenticated = login(account)
    token = authenticated['access_token']
    master = derive_master(account['password'], account['email'], account['kdf'])
    phash = password_hash(master, account['password'])
    _, response = request('/api/two-factor/get-authenticator', token=token, data={'masterPasswordHash': phash})
    if response['enabled']:
        raise FixtureError('A fresh fixture account unexpectedly has an enabled authenticator')
    seed = response['key']
    # Activation consumes a successful time step too. The server's standard
    # one-step drift allowance lets activation use the previous step; the first
    # login can then test a distinct current code immediately.
    if 30 - time.time() % 30 < 1:
        wait_next_totp_step()
    _, enabled = request('/api/two-factor/authenticator', token=token, data={
        'masterPasswordHash': phash, 'key': seed, 'token': totp(seed, time.time() - 30),
    })
    if enabled.get('enabled') is not True:
        raise FixtureError('The dedicated authenticator fixture was not enabled')
    account['twoFactorSeed'] = seed


def verified_login(account: dict) -> dict:
    seed = account.get('twoFactorSeed')
    if seed is None:
        _, authenticated = login(account)
        return authenticated
    status, challenge = login(account, expected_error=True)
    if status != 400 or not isinstance(challenge, dict):
        raise FixtureError('The real authenticator fixture did not require a second factor')
    providers = challenge.get('TwoFactorProviders2', challenge.get('twoFactorProviders2', {}))
    if not isinstance(providers, dict) or '0' not in providers:
        raise FixtureError('The authenticator challenge did not advertise provider 0')
    # Seven digits can never equal this fixture's six-digit SHA-1 TOTP; the
    # request still reaches the server's real numeric-code validation path.
    status, _ = login(account, two_factor='9999999', expected_error=True)
    if status != 400:
        raise FixtureError('The real server accepted an invalid authenticator code')
    status, authenticated = login(account, two_factor=totp(seed), expected_error=True)
    if status != 200:
        # A prior verification or native/Rust test can already have consumed the
        # current time step. Retry once after an actual step change only.
        wait_next_totp_step()
        status, authenticated = login(account, two_factor=totp(seed), expected_error=True)
    if status != 200 or not isinstance(authenticated, dict) or not authenticated.get('access_token'):
        raise FixtureError('The real server rejected a fresh valid authenticator code')
    account['twoFactorLastVerifiedStep'] = int(time.time() // 30)
    return authenticated


def create_account(label: str, kdf: dict, run: str) -> dict:
    email = f'veyra-fixture-{label.lower()}-{run}@veyra.test'
    password = secrets.token_urlsafe(28)
    master = derive_master(password, email, kdf)
    user_key = secrets.token_bytes(64)
    protected_key = encrypt(user_key, stretch(master))
    private = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    private_der = private.private_bytes(serialization.Encoding.DER, serialization.PrivateFormat.PKCS8, serialization.NoEncryption())
    public_der = private.public_key().public_bytes(serialization.Encoding.DER, serialization.PublicFormat.SubjectPublicKeyInfo)
    _, verification_token = request('/identity/accounts/register/send-verification-email', data={'email': email, 'name': 'Veyra fixture'})
    if not isinstance(verification_token, str) or not verification_token:
        raise FixtureError('Test registration must return its verification token; disable signup email verification on the dedicated server')
    request('/identity/accounts/register/finish', data={
        'emailVerificationToken': verification_token,
        'email': email, **kdf, 'key': protected_key,
        'masterPasswordHash': password_hash(master, password),
        'keys': {'publicKey': b64(public_der), 'encryptedPrivateKey': encrypt(private_der, user_key)},
    })
    account = {'label': label, 'email': email, 'password': password, 'kdf': kdf}
    _, logged_in = login(account)
    token = logged_in['access_token']
    assert decrypt(logged_in['Key'], stretch(master)) == user_key
    _, sync = request('/api/sync?excludeDomains=true', token=token)
    user_id = sync['profile']['id']
    expected_items = []
    for per_cipher in (False, True):
        suffix = 'item-key' if per_cipher else 'user-key'
        data_key = secrets.token_bytes(64) if per_cipher else user_key
        fields = {
            'name': f'Veyra integration {label} {suffix}',
            'username': 'fixture-user@veyra.test',
            'password': 'Only-a-test-' + secrets.token_urlsafe(18),
            'uri': 'https://example.com/veyra-integration',
            'totp': 'GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ', # RFC 6238 SHA-1 test secret
            'notes': 'Synthetic integration fixture. Never use with a real account.',
        }
        _, created = request('/api/ciphers', token=token, data={
            'type': 1, 'encryptedFor': user_id, 'organizationId': None, 'folderId': None,
            'name': encrypt(fields['name'], data_key), 'notes': encrypt(fields['notes'], data_key),
            'key': encrypt(data_key, user_key) if per_cipher else None,
            'favorite': True, 'reprompt': 0, 'fields': [],
            'login': {'username': encrypt(fields['username'], data_key), 'password': encrypt(fields['password'], data_key),
                      'totp': encrypt(fields['totp'], data_key),
                      'uris': [{'uri': encrypt(fields['uri'], data_key), 'match': None}]},
        })
        expected_items.append({'cipherId': created['id'], 'perCipherKey': per_cipher, **fields})
    account['expected'] = expected_items[0]
    account['items'] = expected_items
    verify_account(account)
    return account


def verify_account(account: dict) -> None:
    _, prelogin = request('/identity/accounts/prelogin', data={'email': account['email']})
    assert all(prelogin.get(k) == v for k, v in account['kdf'].items())
    status, rejected = login(account, wrong=True)
    assert status == 400 and rejected is None, 'A wrong authorization hash was not rejected'
    logged_in = verified_login(account)
    token = logged_in['access_token']
    master = derive_master(account['password'], account['email'], account['kdf'])
    user_key = decrypt(logged_in['Key'], stretch(master))
    _, sync = request('/api/sync?excludeDomains=true', token=token)
    by_id = {item['id']: item for item in sync['ciphers']}
    for expected in account['items']:
        item = by_id[expected['cipherId']]
        key = decrypt(item['key'], user_key) if item.get('key') else user_key
        assert decrypt(item['name'], key).decode() == expected['name']
        assert decrypt(item['notes'], key).decode() == expected['notes']
        for field in ('username', 'password', 'totp'):
            assert decrypt(item['login'][field], key).decode() == expected[field]
        assert decrypt(item['login']['uris'][0]['uri'], key).decode() == expected['uri']


def print_fixture_summary(manifest: dict, action: str) -> None:
    accounts = manifest['accounts']
    methods = len({account['kdf']['kdf'] for account in accounts})
    items = sum(len(account['items']) for account in accounts)
    factors = sum('twoFactorSeed' in account for account in accounts)
    print(f'Dedicated fixtures {action}: {len(accounts)} accounts, {methods} KDFs, {items} encrypted ciphers, {factors} authenticator accounts; wrong-password rejection verified.')


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server-url', default='http://127.0.0.1:8222', help='Dedicated literal-loopback Vaultwarden test origin')
    parser.add_argument('--output', required=True, type=Path, help='0600 credential manifest outside the checkout')
    parser.add_argument('--reuse', action='store_true', help='Revalidate an existing fixture manifest instead of creating fresh accounts')
    parser.add_argument('--server-version', default='unknown', help='Version from the pinned container image, not the Bitwarden-compatible API version')
    args = parser.parse_args()
    global SERVER, OUTPUT
    parsed = urllib.parse.urlsplit(args.server_url)
    try:
        literal_loopback = ipaddress.ip_address(parsed.hostname or '').is_loopback
    except ValueError:
        literal_loopback = False
    if parsed.scheme != 'http' or not literal_loopback or parsed.username or parsed.password or parsed.query or parsed.fragment or parsed.path not in ('', '/'):
        raise FixtureError('Only a literal-loopback HTTP test server origin is permitted')
    SERVER = args.server_url.rstrip('/')
    if args.output.is_symlink():
        raise FixtureError('The fixture output must not be a symbolic link')
    OUTPUT = args.output.resolve()
    checkout = Path(__file__).resolve().parents[2]
    if OUTPUT.is_relative_to(checkout):
        raise FixtureError('The credential manifest must be outside the repository checkout')
    if OUTPUT.exists() and not args.reuse:
        raise FixtureError('The fixture output already exists; use --reuse to verify it or select a new output path')
    OUTPUT.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    check_sdk_vectors()
    if OUTPUT.exists() and args.reuse:
        if OUTPUT.stat().st_mode & 0o077:
            raise FixtureError('Refusing to read a credential manifest accessible to other users')
        manifest = json.loads(OUTPUT.read_text())
        if manifest.get('schemaVersion') != 1 or manifest.get('serverUrl') != SERVER:
            raise FixtureError('The credential manifest belongs to another test schema or server')
        for account in manifest['accounts']:
            verify_account(account)
        print_fixture_summary(manifest, 'verified')
        return
    run = uuid.uuid4().hex[:12]
    accounts = [
        create_account('PBKDF2', {'kdf': 0, 'kdfIterations': 600000, 'kdfMemory': None, 'kdfParallelism': None}, run),
        create_account('Argon2id', {'kdf': 1, 'kdfIterations': 3, 'kdfMemory': 64, 'kdfParallelism': 4}, run),
    ]
    authenticator = create_account('Authenticator', {'kdf': 0, 'kdfIterations': 600000, 'kdfMemory': None, 'kdfParallelism': None}, run)
    enable_authenticator(authenticator)
    verify_account(authenticator)
    accounts.append(authenticator)
    _, config = request('/api/config')
    version = config.get('version', 'unknown') if isinstance(config, dict) else 'unknown'
    manifest = {'schemaVersion': 1, 'serverUrl': SERVER, 'vaultwardenVersion': args.server_version, 'serverCompatibilityVersion': version, 'accounts': accounts}
    fd = os.open(OUTPUT, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    os.fchmod(fd, 0o600)
    with os.fdopen(fd, 'w') as handle:
        json.dump(manifest, handle, indent=2)
        handle.write('\n')
    print_fixture_summary(manifest, 'created and verified')
    print('Credential manifest stored with mode 0600 outside the repository.')


if __name__ == '__main__':
    try:
        main()
    except (FixtureError, AssertionError, KeyError, ValueError, OSError) as exc:
        # Exception data may contain upstream data: report only explicitly safe errors.
        print(str(exc) if isinstance(exc, FixtureError) else 'Fixture validation failed; no credential data was logged.', file=sys.stderr)
        sys.exit(1)
