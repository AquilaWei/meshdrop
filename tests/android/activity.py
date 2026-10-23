"""Match resumed activity and focused window before dispatching native input."""
import re


def activity_class(component):
    package, _, name = component.partition('/')
    return package + name if name.startswith('.') else name


def foreground_activity(activities, windows):
    """Return the focused resumed class, or None while a transition is incomplete."""
    resumed = re.search(r'(?:topResumedActivity|mResumedActivity|ResumedActivity)[:=]\s*ActivityRecord\{\S+ u\d+ (\S+)', activities)
    focused = re.search(r'mCurrentFocus=Window\{\S+ u\d+ ([^\s}]+)', windows)
    if not resumed or not focused:
        return None
    resumed_class = activity_class(resumed[1])
    if resumed_class != activity_class(focused[1]):
        return None
    return resumed_class
