"""Native chooser transitions must finish before the scenario sends Back."""
import unittest

from activity import foreground_activity


class ForegroundActivityTests(unittest.TestCase):
    def test_pending_chooser_intent_does_not_establish_foreground_chooser(self):
        activities = '''Intent { act=android.intent.action.CHOOSER }
        ActivityRecord{123 u0 com.android.intentresolver/.ChooserActivity t8}
        mResumedActivity: ActivityRecord{456 u0 org.meshdrop.desktop/.MainActivity t8}'''
        windows = 'mCurrentFocus=Window{789 u0 org.meshdrop.desktop/org.meshdrop.desktop.MainActivity}'
        self.assertEqual(foreground_activity(activities, windows), 'org.meshdrop.desktop.MainActivity')

    def test_api34_equals_format_reports_focused_resumed_activity(self):
        activities = 'topResumedActivity=ActivityRecord{f473576 u0 org.meshdrop.desktop/.MainActivity t8}'
        windows = 'mCurrentFocus=Window{37da2cf u0 org.meshdrop.desktop/org.meshdrop.desktop.MainActivity}'
        self.assertEqual(foreground_activity(activities, windows), 'org.meshdrop.desktop.MainActivity')

    def test_resumed_chooser_without_window_focus_is_not_ready(self):
        activities = 'topResumedActivity: ActivityRecord{123 u0 com.android.intentresolver/.ChooserActivity t8}'
        windows = 'mCurrentFocus=Window{789 u0 org.meshdrop.desktop/org.meshdrop.desktop.MainActivity}'
        self.assertIsNone(foreground_activity(activities, windows))

    def test_resumed_and_focused_chooser_is_ready(self):
        activities = 'mResumedActivity: ActivityRecord{123 u0 com.android.intentresolver/.ChooserActivity t8}'
        windows = 'mCurrentFocus=Window{789 u0 com.android.intentresolver/com.android.intentresolver.ChooserActivity}'
        self.assertEqual(foreground_activity(activities, windows), 'com.android.intentresolver.ChooserActivity')

    def test_null_focus_during_transition_is_not_ready(self):
        activities = 'mResumedActivity: ActivityRecord{123 u0 com.android.intentresolver/.ChooserActivity t8}'
        self.assertIsNone(foreground_activity(activities, 'mCurrentFocus=null'))

    def test_main_activity_resumed_behind_focused_chooser_is_not_ready(self):
        activities = 'mResumedActivity: ActivityRecord{123 u0 org.meshdrop.desktop/.MainActivity t8}'
        windows = 'mCurrentFocus=Window{789 u0 com.android.intentresolver/com.android.intentresolver.ChooserActivity}'
        self.assertIsNone(foreground_activity(activities, windows))


if __name__ == '__main__':
    unittest.main()
