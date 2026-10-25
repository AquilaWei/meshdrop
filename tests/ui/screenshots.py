"""Deterministic preview data in the real Tauri webview; no transport claims."""
import base64
import os
from pathlib import Path
import tempfile

import scenario as ui

THEME = os.environ.get('MESHDROP_SCREEN_THEME', 'light')
OUTPUT = Path('/tmp/meshdrop-016-screens')
OUTPUT.mkdir(exist_ok=True)

with tempfile.TemporaryDirectory(prefix='meshdrop-preview-') as folder:
    profile = Path(folder) / 'profile'
    ui.config(profile, 'My Mac', 19853)
    try:
        ui.launch(['tauri-driver'], env=dict(os.environ, MESHDROP_STATE_DIR=str(profile)))
        ui.wait(lambda: ui.http('GET', '/status'))
        ui.session = ui.http('POST', '/session', {'capabilities':{'alwaysMatch':{'tauri:options':{'application':str(Path('target/debug/meshdrop-app').resolve())}}}})['sessionId']
        ui.wait(lambda: 'Receiving on port' in ui.text('#service-state'))
        ui.wait(lambda: ui.js('return !refreshing'))
        dark = ui.js("return matchMedia('(prefers-color-scheme: dark)').matches")
        assert dark == (THEME == 'dark'), f'Native theme preference {dark} differs from {THEME}'
        ui.js("""clearInterval(pollingTimer); clearInterval(discoveryTimer);
        poll = async () => { devices(preview.devices); invitations(preview.invitations); transfers(preview.transfers); chosenFiles=preview.selected_files; renderFiles(); };
        window.preview = {name:'My Mac',platform:'linux',port:19853,fingerprint:'12:34:56:78:90:AB:CD:EF',qr_content:'meshdrop://192.168.1.10:9843',qr:[[true,false],[false,true]],problems:[],selected_files:['Summer photos.zip'],invitations:[],
        devices:[{name:'Alex’s MacBook',platform:'macos',online:true,paired:true,target:'alex',addresses:[]},{name:'Sam’s phone',platform:'android',online:true,paired:false,target:'sam',addresses:[]},{name:'Work PC',platform:'windows',online:true,paired:true,target:'work',addresses:[]}],
        transfers:[{id:'preview',direction:'send',peer:'alex',status:'accepted',files:[{name:'Summer photos.zip',bytes:42000000,total:100000000,status:'accepted'}]}]};
        return true;""")
        for width in (1040, 390):
            ui.http('POST', f'/session/{ui.session}/window/rect', {'width':width,'height':850,'x':0,'y':0})
            for screen in ('main', 'searching', 'incoming', 'pairing', 'settings', 'settings-network'):
                ui.js("""document.querySelectorAll('dialog[open]').forEach(dialog => dialog.close());
                    showPage(false); document.querySelector('#refresh').setAttribute('aria-busy','false');
                    preview.invitations=[]; lastInvitations=''; poll(); return true;""")
                ui.wait(lambda: 'Alex' in ui.text('#devices'))
                if screen == 'searching':
                    ui.js("document.querySelector('#refresh').setAttribute('aria-busy','true'); return true")
                if screen == 'incoming':
                    ui.js("preview.invitations=[{id:'incoming',kind:'files',name:'Sam’s phone',peer:'sam',detail:{files:[{name:'Weekend.jpg',size:2400000},{name:'Notes.pdf',size:120000}]}}]; invitations(preview.invitations); return true")
                if screen == 'pairing':
                    ui.js("document.querySelector('#pair-name').textContent='Sam’s phone'; document.querySelector('#pair-avatar').textContent='SP'; document.querySelector('#pair-code').textContent='482916'; document.querySelector('#outgoing-pair').showModal(); return true")
                if screen == 'settings':
                    ui.js('showPage(true); return true')
                if screen == 'settings-network':
                    ui.js("showPage(true); openSettings('network'); return true")
                # Capture only after the webview has committed its next frame.
                ui.js('return document.body.offsetHeight')
                image = ui.http('GET', f'/session/{ui.session}/screenshot')
                (OUTPUT / f'{screen}-{width}-{THEME}.png').write_bytes(base64.b64decode(image))
        print(f'PASS: native {THEME} mode and 12 preview screenshots', flush=True)
    finally:
        if ui.session:
            ui.http('DELETE', f'/session/{ui.session}')
        for process in reversed(ui.processes):
            process.terminate()
            process.wait(timeout=10)
