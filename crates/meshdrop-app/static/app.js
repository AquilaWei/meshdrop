'use strict';
const request = (operation, payload = null) => window.__TAURI__.core.invoke('desktop_request', {operation, payload});
const $ = selector => document.querySelector(selector);
const selected = new Set();
let pairing = null;
let refreshing = false;
let androidStarted = false;
let lastDevices = '';
let lastInvitations = '';
function element(tag, content, className) {
  const node = document.createElement(tag);
  if (content !== undefined) node.textContent = String(content);
  if (className) node.className = className;
  return node;
}
function failure(error) {
  $('#error').textContent = String(error);
  $('#error').hidden = false;
  $('#fingerprint-error').hidden = !String(error).toLowerCase().includes('fingerprint');
}
async function act(operation, payload) {
  $('#error').hidden = true;
  try { return await request(operation, payload); }
  catch (error) { failure(error); throw error; }
}
async function refresh() {
  if (refreshing) return;
  refreshing = true;
  $('#refresh').disabled = true;
  try { await act('refresh'); await poll(); }
  catch (_) { /* act renders the failure; periodic refresh remains available. */ }
  finally { refreshing = false; $('#refresh').disabled = false; }
}
async function preparePair(address) {
  try {
    pairing = await act('prepare_pair', {address});
    $('#pair-name').textContent = pairing.name;
    $('#pair-code').textContent = pairing.code;
    $('#pair-status').textContent = '';
    $('#outgoing-pair').hidden = false;
    $('#pair-confirm').disabled = false;
  } catch (_) { /* The command error remains visible until the next action. */ }
}
function devices(items) {
  const signature = JSON.stringify(items);
  if (signature === lastDevices) return;
  lastDevices = signature;
  $('#devices').replaceChildren();
  for (const device of items) {
    const row = element('div', undefined, 'device');
    const checkbox = element('input');
    checkbox.type = 'checkbox'; checkbox.disabled = !device.paired || !device.online;
    checkbox.checked = selected.has(device.target); checkbox.setAttribute('aria-label', `Select ${device.name}`);
    checkbox.addEventListener('change', () => checkbox.checked ? selected.add(device.target) : selected.delete(device.target));
    const description = element('div', undefined, 'description');
    description.append(element('strong', device.name), element('small', `${device.platform} · ${device.online ? 'Online' : 'Offline'} · ${device.paired ? 'Paired' : 'Unpaired'}`), element('small', device.addresses.join(', ')));
    row.append(checkbox, description);
    const button = element('button', device.mismatch ? 'Fingerprint mismatch' : 'Pair');
    button.disabled = device.paired || !device.online;
    button.addEventListener('click', () => preparePair(device.addresses[0]));
    row.append(button); $('#devices').append(row);
  }
  if (!items.length) $('#devices').append(element('p', 'No devices yet. Refresh or add an address.'));
}
function invitations(items) {
  const signature = JSON.stringify(items);
  if (signature === lastInvitations) return;
  lastInvitations = signature;
  $('#invitations').replaceChildren();
  for (const invitation of items) {
    const box = element('div', undefined, 'invitation'); box.setAttribute('role','dialog');
    box.append(element('h3', `${invitation.kind === 'pair' ? 'Pair with' : 'Receive from'} ${invitation.name || invitation.peer}`));
    if (invitation.kind === 'pair') box.append(element('strong', invitation.detail.code, 'code'));
    else for (const file of invitation.detail.files) box.append(element('p', `${file.name} · ${file.size} bytes`));
    for (const accept of [true, false]) {
      const button = element('button', accept ? 'Accept' : 'Reject');
      button.dataset.action = accept ? 'accept' : 'reject';
      button.addEventListener('click', async () => {
        button.disabled = true;
        try { await act('decide', {id:invitation.id, accept}); await poll(); } catch (_) { button.disabled = false; }
      }); box.append(button);
    }
    $('#invitations').append(box);
  }
}
let lastTransfers = '';
function transfers(items) {
  const signature = JSON.stringify(items);
  if (signature === lastTransfers) return;
  lastTransfers = signature; $('#transfers').replaceChildren();
  for (const transfer of items) {
    const box = element('div', undefined, 'transfer'); box.dataset.id = transfer.id;
    box.append(element('h3', `${transfer.direction === 'send' ? 'Sending to' : 'Receiving from'} ${transfer.peer}`));
    const status = element('p', transfer.status.replaceAll('_',' '), 'transfer-status'); box.append(status);
    for (const file of transfer.files) {
      const percent = file.status === 'complete' ? 100 : file.total ? Math.floor(100*file.bytes/file.total) : 0;
      box.append(element('p', `${file.name} · ${percent}% · ${file.bytes}/${file.total} bytes`));
      const bar = element('progress'); bar.max = 100; bar.value = percent; bar.setAttribute('aria-label', file.name); box.append(bar);
    }
    if (transfer.error) box.append(element('p', transfer.error));
    if (['waiting','accepted'].includes(transfer.status)) {
      const cancel = element('button','Cancel','secondary');
      cancel.addEventListener('click', async () => { try { await act('cancel',{id:transfer.id}); await poll(); } catch (_) { /* rendered */ } }); box.append(cancel);
    }
    $('#transfers').append(box);
  }
}
function drawQR(matrix) {
  const canvas = $('#qr');
  const scale = 4, quiet = 4;
  canvas.width = canvas.height = (matrix.length + 2 * quiet) * scale;
  const context = canvas.getContext('2d'); context.fillStyle = 'white'; context.fillRect(0,0,canvas.width,canvas.height);
  context.fillStyle = '#152d3d';
  matrix.forEach((row,y) => row.forEach((dark,x) => { if (dark) context.fillRect((x+quiet)*scale,(y+quiet)*scale,scale,scale); }));
}
async function poll() {
  try {
    const state = await request('snapshot');
    $('#my-name').textContent = state.name;
    if (state.platform === 'android') {
      $('#android-controls').hidden = false;
      if (!androidStarted) { androidStarted = true; startAndroid(); }
      $('#setting-folder').readOnly = true;
      document.querySelector('footer').textContent = 'Background reception uses the ongoing notification. Stop it explicitly to close the listener.';
      $('#open-folder').disabled = true; $('#open-folder').title = 'Desktop-only. Android receives into app-private storage.';
      const tailscale = document.querySelector('[name=provider][value=tailscale]');
      tailscale.disabled = true; tailscale.parentNode.title = 'Desktop-only. Use manual VPN addresses on Android.';
    }
    $('#fingerprint').textContent = state.fingerprint;
    $('#qr-content').textContent = state.qr_content;
    $('#service-state').textContent = `Receiving on port ${state.port}`;
    drawQR(state.qr); devices(state.devices); invitations(state.invitations); transfers(state.transfers);
    $('#selected-files').textContent = (state.selected_files || []).join(', ') || 'No files selected';
    $('#problems').replaceChildren(...state.problems.map(problem => element('li', `${problem.provider}: ${problem.error}`)));
    if (state.error) failure(state.error);
  } catch (error) { failure(error); }
}
$('#manual').addEventListener('submit', async event => {
  event.preventDefault();
  try { await act('add_device', {address:$('#address').value}); $('#address').value = ''; await refresh(); } catch (_) { /* rendered */ }
});
$('#refresh').addEventListener('click', refresh);
$('#pair-dismiss').addEventListener('click', () => { pairing = null; $('#outgoing-pair').hidden = true; });
$('#pair-confirm').addEventListener('click', async () => {
  $('#pair-confirm').disabled = true; $('#pair-status').textContent = 'Waiting for the other device to accept…';
  try { await act('confirm_pair', pairing); $('#pair-status').textContent = 'Paired'; await refresh(); }
  catch (_) { $('#pair-status').textContent = 'Pairing failed'; }
});
poll(); refresh(); setInterval(poll, 300); setInterval(refresh, 8000);

$('#pick-files').addEventListener('click', async () => { try { await act('pick_files'); await poll(); } catch (_) { /* rendered */ } });
$('#send').addEventListener('click', async () => { try { await act('send',{targets:[...selected]}); await poll(); } catch (_) { /* rendered */ } });
$('#open-folder').addEventListener('click', async () => { try { await act('open_receive_directory'); } catch (_) { /* rendered */ } });

let settings = null;
function renderSettings(value) {
  settings = value.config;
  $('#setting-folder').value = settings.receive_directory; $('#setting-port').value = settings.port;
  $('#setting-networks').value = settings.allowed_networks.join('\n');
  document.querySelectorAll('[name=provider]').forEach(node => { node.checked = settings.providers.includes(node.value); });
  $('#setting-invitation').value = settings.invitation_timeout_seconds; $('#setting-idle').value = settings.idle_timeout_seconds;
  $('#setting-notifications').checked = settings.notifications_enabled;
  $('#settings-toml').value = value.toml;
}
async function saveSettings(payload) {
  try { renderSettings(await act('save_settings',payload)); $('#settings-status').textContent = 'Saved. Restart to apply folder and network settings; notifications updated now.'; }
  catch (_) { $('#settings-status').textContent = 'Settings were not saved.'; }
}
$('#settings-form').addEventListener('submit', event => {
  event.preventDefault();
  saveSettings({config:{...settings, receive_directory:$('#setting-folder').value, port:Number($('#setting-port').value), allowed_networks:$('#setting-networks').value.split(/\s+/).filter(Boolean), providers:[...document.querySelectorAll('[name=provider]:checked')].map(node => node.value), invitation_timeout_seconds:Number($('#setting-invitation').value), idle_timeout_seconds:Number($('#setting-idle').value), notifications_enabled:$('#setting-notifications').checked}});
});
$('#save-toml').addEventListener('click', () => saveSettings({toml:$('#settings-toml').value}));
act('settings').then(renderSettings).catch(() => { /* act renders configuration errors */ });

async function receivedFiles() {
  const result = await act('received_files');
  $('#received-files').replaceChildren();
  for (const name of result.files) {
    const row = element('div'); row.append(element('span', name));
    const share = element('button','Share');
    share.addEventListener('click', () => act('share_file',{name}).catch(() => {}));
    row.append(share); $('#received-files').append(row);
  }
}
async function startAndroid() {
  try {
    const result = await act('android_start');
    $('#android-permission').textContent = result.permission === 'granted'
      ? 'Background reception enabled. Incoming invitations require your acceptance.'
      : 'Notifications denied. Enable notifications in Android Settings to see incoming alerts; invitations still require acceptance in the app.';
    await receivedFiles();
  } catch (_) { $('#android-permission').textContent = 'Background reception could not start. Keep the app visible and review the error.'; }
}
$('#received-refresh').addEventListener('click', () => receivedFiles().catch(() => {}));
$('#stop-receiver').addEventListener('click', async () => {
  try { await act('android_stop'); $('#service-state').textContent = 'Reception stopped'; $('#android-permission').textContent = 'Reception stopped. Reopen the app to start again.'; }
  catch (_) { /* displayed */ }
});

$('#scan-qr').addEventListener('click', async () => {
  try {
    const permission = await window.__TAURI__.core.invoke('plugin:barcode-scanner|request_permissions');
    if (permission.camera !== 'granted') throw new Error('Camera permission denied. Enter a host:port or paste QR content manually.');
    const result = await window.__TAURI__.core.invoke('plugin:barcode-scanner|scan',{formats:['QR_CODE'],windowed:false});
    if (!result.content || !result.content.startsWith('meshdrop://')) throw new Error('This is not a meshdrop pairing QR. Enter an address manually.');
    await act('add_device',{address:result.content}); await refresh();
    await preparePair(result.content);
  } catch (error) { failure(error); }
});
