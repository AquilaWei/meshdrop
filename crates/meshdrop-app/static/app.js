'use strict';
const request = (operation, payload = null) => window.__TAURI__.core.invoke('desktop_request', {operation, payload});
request('snapshot').then(state => {
  document.querySelector('#identity').textContent = `${state.name} — ${state.fingerprint}`;
}).catch(error => { document.querySelector('#error').textContent = String(error); });
