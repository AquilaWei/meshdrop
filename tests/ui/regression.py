"""Real-webview discovery regressions and frontend-only poll/render profiling."""
import json
import os
from pathlib import Path
import tempfile
import time

import scenario as ui


def main():
    failures = []
    with tempfile.TemporaryDirectory(prefix='meshdrop-regression-') as folder:
        profile = Path(folder) / 'profile'
        ui.config(profile, 'Regression device', 19853)
        ui.launch(['tauri-driver'], env=dict(os.environ, MESHDROP_STATE_DIR=str(profile)))
        ui.wait(lambda: ui.http('GET', '/status'))
        ui.session = ui.http('POST', '/session', {'capabilities': {'alwaysMatch': {'tauri:options': {'application': str(Path('target/debug/meshdrop-app').resolve())}}}})['sessionId']
        ui.wait(lambda: 'Receiving on port' in ui.text('#service-state'))
        ui.wait(lambda: ui.js('return !refreshing'))
        ui.js("""window.metrics = {requests:{}, poll:[], render:[], ticks:[], qrRender:[], mutations:0, grid:0, qr:0};
            const originalRequest = request;
            // act is mutable; snapshot timings below include the real IPC/actor.
            window.originalAct = act;
            act = (op,payload) => { metrics.requests[op]=(metrics.requests[op]||0)+1; return originalAct(op,payload); };
            window.pollObserver = new MutationObserver(changes => metrics.mutations += changes.length);
            pollObserver.observe(document.querySelector('main'),{subtree:true,childList:true,attributes:true,characterData:true});
            window.gridObserver = new MutationObserver(changes => metrics.grid += changes.length);
            gridObserver.observe(document.querySelector('#devices'),{subtree:true,childList:true,attributes:true,characterData:true});
            if (typeof updateSnapshot === 'function') {
                const originalUpdate = updateSnapshot;
                updateSnapshot = () => { metrics.ticks.push(performance.now()); return originalUpdate(); };
            }
            window.originalQR = drawQR;
            drawQR = matrix => {
                if (typeof updateSnapshot !== 'function') metrics.ticks.push(performance.now());
                metrics.qr++; const start=performance.now(); const result=originalQR(matrix);
                metrics.qrRender.push(performance.now()-start); return result;
            };
            window.originalPoll = poll;
            poll = async () => { const start=performance.now(); await originalPoll(); metrics.poll.push(performance.now()-start); };
            // Force only the visibility getter, retaining the real event listener/timers.
            Object.defineProperty(document,'hidden',{configurable:true,get:()=>Boolean(window.testHidden)});
            Object.defineProperty(document,'visibilityState',{configurable:true,get:()=>window.testHidden?'hidden':'visible'});
            window.testHidden=false;
            document.dispatchEvent(new Event('visibilitychange'));
            window.originalDevices=devices;
            devices = items => { const start=performance.now(); const result=originalDevices(items); metrics.render.push(performance.now()-start); return result; };
            window.measurementStart=performance.now(); refresh(); metrics.requests.refresh=0;
            return true;""")
        # More than three old 8s searches, still before the new 30s deadline.
        time.sleep(29)
        calls = ui.js('return metrics.requests.refresh || 0')
        if calls != 0:
            failures.append(f'no second automatic search within 30 s: got {calls} additional searches in 29 s')
        else:
            print('PASS: no second automatic search within 30 s', flush=True)
        ui.wait(lambda: ui.js('return (metrics.requests.refresh || 0) >= 1'), timeout=6)
        print('PASS: foreground automatic search runs at the 30 s deadline', flush=True)
        ui.wait(lambda: ui.js('return !refreshing'))
        ui.js("window.testHidden=true; document.dispatchEvent(new Event('visibilitychange')); metrics.requests.refresh=0; return true")
        time.sleep(31)
        calls = ui.js('return metrics.requests.refresh || 0')
        if calls != 0:
            failures.append(f'no search while hidden: got {calls} searches in 31 s')
        else:
            print('PASS: no search while hidden', flush=True)
        ui.wait(lambda: ui.js('return !refreshing'))
        print('IDLE ' + json.dumps(ui.js('return {durationMs:performance.now()-measurementStart,snapshotTicks:metrics.ticks,qrDraws:metrics.qr}')), flush=True)
        ui.js("""clearInterval(pollingTimer); clearInterval(discoveryTimer);
            window.testDevice={name:'Same device',target:'test-peer',pairing_address:'test-peer',platform:'linux',paired:true,online:true,mismatch:false,addresses:['host-a','host-b']};
            devices([testDevice]); window.savedDevice=document.querySelector('#devices .device');
            devices([{...testDevice,addresses:['host-b','host-a']}]); return true;""")
        if not ui.js("return document.querySelector('#devices .device') === savedDevice"):
            failures.append('unchanged rendered devices preserve grid DOM: address metadata replaced the device node')
        else:
            print('PASS: unchanged rendered devices preserve grid DOM', flush=True)
        # Measure an unchanged real snapshot, excluding discovery and test fixtures.
        ui.js('window.profileDone=false; metrics.poll=[]; metrics.render=[]; metrics.qrRender=[]; metrics.ticks=[]; metrics.mutations=0; metrics.grid=0; metrics.qr=0; (async()=>{await poll(); metrics.mutations=0; metrics.grid=0; metrics.qr=0; metrics.poll=[]; metrics.render=[]; for(let i=0;i<100;i++) await poll(); window.profileDone=true;})(); return true')
        ui.wait(lambda: ui.js('return profileDone'))
        result = ui.js('return metrics')
        assert result['mutations'] == 0 or failures, 'Unchanged snapshots mutated the DOM'
        assert result['qr'] == 0 or failures, 'Unchanged snapshots redrew the QR'
        print('PROFILE ' + json.dumps(result), flush=True)
        for failure in failures:
            print('FAIL: ' + failure, flush=True)
        if failures:
            raise AssertionError(f'{len(failures)} discovery/render regressions failed')


if __name__ == '__main__':
    try:
        main()
    finally:
        if ui.session:
            ui.http('DELETE', f'/session/{ui.session}')
        for process in reversed(ui.processes):
            process.terminate()
            process.wait(timeout=10)
