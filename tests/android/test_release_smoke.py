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


class AppRetentionTests(unittest.TestCase):
    def test_disappeared_app_returns_empty_pid(self):
        with patch.object(release_smoke, 'adb', side_effect=subprocess.CalledProcessError(1, 'pidof')):
            self.assertEqual(release_smoke.app_pid(), b'')

    def test_retention_adb_transport_failure_is_propagated(self):
        with patch.object(release_smoke, 'adb', side_effect=subprocess.CalledProcessError(2, 'adb')):
            with self.assertRaises(subprocess.CalledProcessError):
                release_smoke.app_pid()

    def test_retained_app_returns_trimmed_pid(self):
        with patch.object(release_smoke, 'adb', return_value=b'5552\n'):
            self.assertEqual(release_smoke.app_pid(), b'5552')

    def test_exit_one_with_adb_error_is_not_process_disappearance(self):
        error = subprocess.CalledProcessError(1, 'adb', stderr=b'error: device offline')
        with patch.object(release_smoke, 'adb', side_effect=error):
            with self.assertRaises(subprocess.CalledProcessError):
                release_smoke.app_pid()
