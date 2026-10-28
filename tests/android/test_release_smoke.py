"""First-boot readiness must tolerate a provider that has not started yet."""
import os
import subprocess
import unittest
from unittest.mock import patch

os.environ.setdefault('ANDROID_HOME', '/unused-sdk')
import release_smoke


class ProviderReadinessTests(unittest.TestCase):
    def test_absent_provider_returns_empty_pid(self):
        with patch.object(release_smoke, 'adb', side_effect=subprocess.CalledProcessError(1, 'pidof')):
            self.assertEqual(release_smoke.provider_pid(), b'')

    def test_adb_transport_failure_is_propagated(self):
        with patch.object(release_smoke, 'adb', side_effect=subprocess.CalledProcessError(2, 'adb')):
            with self.assertRaises(subprocess.CalledProcessError):
                release_smoke.provider_pid()

    def test_started_provider_returns_trimmed_pid(self):
        with patch.object(release_smoke, 'adb', return_value=b'1234\n'):
            self.assertEqual(release_smoke.provider_pid(), b'1234')
