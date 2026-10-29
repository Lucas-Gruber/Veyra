"""Exercise scoped audit decisions in disposable repositories, without running Cargo."""
import copy
from datetime import datetime, timedelta, timezone
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / ".security"))
import check_audit_exception as gate

HERE = Path(__file__).parent
PROJECT = HERE.resolve().parents[1]


class ExceptionGuardTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='veyra-rsa-exception-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        subprocess.run(['git', 'init', '--quiet', str(self.root)], check=True)
        for name in ('package.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock',
                     'src-tauri/tauri.conf.json', 'src-tauri/src/vaultwarden.rs',
                     '.github/workflows/ci.yml'):
            dest = self.root / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(PROJECT/name, dest)
        self.policy = gate.read_json(PROJECT/'.security/rsa-alpha3-policy.json')
        self.report = gate.read_json(HERE/'fixtures/cargo-audit-alpha3.json')
        self.now = datetime(2026, 10, 30, tzinfo=timezone.utc)
        # These approvals exist only in this temporary in-memory fixture.
        self.policy['approval'] = {'approved': True, 'by': 'TEST FIXTURE ONLY',
            'reference': 'synthetic unit-test approval, not an owner decision',
            'on': '2026-10-29'}
        self.bind_source()

    def bind_source(self):
        fingerprint, inventory = gate.source_fingerprint(self.root)
        self.policy['source_fingerprint_sha256'] = fingerprint
        self.policy['protected_source_sha256'] = inventory

    def run_gate(self, **kwargs):
        return gate.evaluate(self.root, self.report, kwargs.get('status', 1),
                             kwargs.get('stderr', b''), self.policy,
                             now=kwargs.get('now', self.now))

    def rejected(self, **kwargs):
        with self.assertRaises((gate.ReviewRequired, ValueError, TypeError, KeyError)):
            self.run_gate(**kwargs)

    def test_exact_synthetic_approval_can_match(self):
        self.assertIn('unresolved', self.run_gate())

    def test_missing_or_future_approval(self):
        self.policy['approval']['approved'] = False
        self.rejected()
        self.policy['approval']['approved'] = True
        self.policy['approval']['on'] = '2026-10-31'
        self.rejected()
        self.policy['approval']['on'] = '2026-10-29'
        self.policy['approval']['reference'] = ''
        self.rejected()

    def test_scope_is_source_bound_without_an_arbitrary_deadline(self):
        self.assertIn('unresolved', self.run_gate(now=self.now+timedelta(days=90)))
        self.policy['reviewed_source_commit'] = '0'*40
        self.rejected()

    def test_wrong_exit_diagnostics_and_empty_report(self):
        for status in (0, 2, -1):
            with self.subTest(status=status): self.rejected(status=status)
        self.rejected(stderr=b'warning: advisory database download failed')
        self.report = {}
        self.rejected()

    def test_filters_and_ignores(self):
        settings = copy.deepcopy(self.report['settings'])
        for key, value in (('ignore', [gate.ADVISORY]), ('target_os',['linux']),
                           ('target_arch',['x86_64']), ('severity','critical'),
                           ('informational_warnings', [])):
            self.report['settings'] = {**settings, key:value}
            with self.subTest(key=key): self.rejected()

    def test_added_or_changed_findings(self):
        finding = self.report['vulnerabilities']['list'][0]
        finding['advisory']['title'] += ' NEW SCOPE'
        self.rejected()
        self.report = gate.read_json(HERE/'fixtures/cargo-audit-alpha3.json')
        self.report['vulnerabilities']['list'].append(copy.deepcopy(finding))
        self.report['vulnerabilities']['count'] = 2
        self.rejected()

    def test_wrong_package_or_checksum(self):
        for key, value in (('version','0.10.0-rc.20'), ('checksum','0'*64), ('name','another-crate')):
            self.report = gate.read_json(HERE/'fixtures/cargo-audit-alpha3.json')
            self.report['vulnerabilities']['list'][0]['package'][key] = value
            with self.subTest(key=key): self.rejected()

    def test_new_warning_and_inconsistent_counts(self):
        self.report['warnings']['notice'] = [copy.deepcopy(self.report['warnings']['unsound'][0])]
        self.report['warnings']['notice'][0]['advisory']['id'] = 'RUSTSEC-2099-9999'
        self.report['warnings']['notice'][0]['advisory']['informational'] = 'notice'
        self.rejected()
        self.report = gate.read_json(HERE/'fixtures/cargo-audit-alpha3.json')
        self.report['vulnerabilities']['found'] = False
        self.rejected()
        self.report = gate.read_json(HERE/'fixtures/cargo-audit-alpha3.json')
        self.report['lockfile']['dependency-count'] = 0
        self.rejected()

    def test_source_and_new_untracked_source(self):
        source = self.root/'src-tauri/src/vaultwarden.rs'
        original = source.read_bytes()
        source.write_bytes(original+b'\n// source changed\n')
        self.rejected()
        source.write_bytes(original)
        (source.parent/'new_security_path.rs').write_text('fn new_path() {}\n')
        self.rejected()

    def test_ignored_source_and_root_cargo_config_are_detected(self):
        (self.root/'.gitignore').write_text('app/hidden.vue\n')
        self.bind_source()
        app = self.root/'app'
        app.mkdir()
        (app/'hidden.vue').write_text('<script>unsafeNewCode()</script>\n')
        self.rejected()
        (self.root/'.gitignore').unlink()
        (app/'hidden.vue').unlink()
        self.bind_source()
        cargo = self.root/'.cargo'
        cargo.mkdir()
        (cargo/'config.toml').write_text('[build]\nrustflags=["--cfg=unsafe"]\n')
        self.rejected()

    def test_generated_cache_is_excluded_but_policy_inventory_cannot_drift(self):
        cache = self.root/'.security/__pycache__'
        cache.mkdir(parents=True)
        (cache/'guard.cpython.pyc').write_bytes(b'generated bytecode')
        self.assertIn('unresolved', self.run_gate())
        self.policy['protected_source_sha256']['src-tauri/src/vaultwarden.rs'] = '0'*64
        self.rejected()

    def test_root_build_controls_and_validation_scripts(self):
        for name in ('Cargo.toml', 'Cargo.lock', '.npmrc', '.pnpmfile.cjs',
                     'pnpmfile.cjs', 'tsconfig.json', 'modules/local.ts',
                     'tests/new_crypto_case.rs', 'scripts/new_check.py'):
            file = self.root/name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text('changed root build or validation control\n')
            with self.subTest(path=name): self.rejected()
            file.unlink()

    def test_changed_sdk_version_and_native_ci(self):
        manifest = self.root/'src-tauri/Cargo.toml'
        original = manifest.read_text()
        manifest.write_text(original.replace(gate.SDK, '0'*40))
        self.rejected()
        manifest.write_text(original)
        ci = self.root/'.github/workflows/ci.yml'
        ci.write_text(ci.read_text()+'\nenv:\n  RUSTFLAGS: --cfg unsafe_new_mode\n')
        self.rejected()

    def test_changed_app_version_lock_or_guard(self):
        package = self.root/'package.json'
        original = package.read_text()
        package.write_text(original.replace(gate.APP,'0.1.0-alpha.4'))
        self.rejected()
        package.write_text(original)
        lock = self.root/'src-tauri/Cargo.lock'
        lock.write_text(lock.read_text()+'\n# changed dependency resolution\n')
        self.rejected()
        self.policy['guard_sha256'] = '0'*64
        self.rejected()

    def test_invalid_database_and_duplicate_json(self):
        for value in ('not-a-timestamp','2999-01-01T00:00:00Z'):
            self.report['database']['last-updated'] = value
            with self.subTest(value=value): self.rejected()
        invalid = self.root/'duplicate.json'
        invalid.write_text('{"approved":false,"approved":true}')
        with self.assertRaises(gate.ReviewRequired): gate.read_json(invalid)
        invalid.write_text('{"x":NaN}')
        with self.assertRaises(gate.ReviewRequired): gate.read_json(invalid)

    def test_clean_future_source_with_new_warning_needs_no_old_policy(self):
        self.report['vulnerabilities'] = {'found':False,'count':0,'list':[]}
        notice = copy.deepcopy(self.report['warnings']['unsound'][0])
        notice['advisory']['id'] = 'RUSTSEC-2099-9999'
        notice['advisory']['informational'] = 'notice'
        self.report['warnings']['notice'] = [notice]
        package = self.root/'package.json'
        package.write_text(package.read_text().replace(gate.APP, '0.1.0-alpha.4'))
        (self.root/'src-tauri/src/vaultwarden.rs').write_text('// independently updated future application\n')
        self.policy = {}
        self.assertIn('no RSA exception', self.run_gate(status=0))

    def test_clean_raw_audit_uses_no_exception(self):
        self.report['vulnerabilities'] = {'found':False,'count':0,'list':[]}
        self.policy['approval']['approved'] = False
        self.assertIn('no RSA exception', self.run_gate(status=0))
        self.rejected(status=1)


if __name__ == '__main__':
    unittest.main()
