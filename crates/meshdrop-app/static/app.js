'use strict';
const request = (operation, payload = null) => window.__TAURI__.core.invoke('desktop_request', {operation, payload});
const $ = selector => document.querySelector(selector);
let chosenFiles = [];
let checkingShares = false;
let pairing = null;
let refreshing = false;
let androidStarted = false;
let receivingStopped = false;
let lastDevices = '';
let lastInvitations = '';
let platform = '';
let currentTransfers = [];
let foreground = true;
let discoveryTimer = null;
let lastSearch = 0;
let pollInFlight = null;
let lastQR = '';
let lastSnapshot = '';
let deviceItems = new Map();
let lastFiles = '';
function element(tag, content, className) {
  const node = document.createElement(tag);
  if (content !== undefined) node.textContent = String(content);
  if (className) node.className = className;
  return node;
}
function failure(error) {
  $('#error').textContent = `${error}. Try again or check config.toml.`;
  setServiceState('Receiver needs attention. Review the error.', true);
  $('#error').hidden = false;
  $('#fingerprint-error').hidden = !String(error).toLowerCase().includes('fingerprint');
}
async function act(operation, payload) {
  if (!$('#error').hidden) $('#error').hidden = true;
  try { return await request(operation, payload); }
  catch (error) { failure(error); throw error; }
}
function canSearch() { return foreground && !document.hidden; }
function scheduleDiscovery() {
  clearTimeout(discoveryTimer);
  discoveryTimer = null;
  if (canSearch()) discoveryTimer = setTimeout(refresh, Math.max(0, 30000 - (Date.now() - lastSearch)));
}
async function refresh() {
  if (refreshing || !canSearch()) return;
  lastSearch = Date.now();
  refreshing = true;
  $('#refresh').setAttribute('aria-busy', 'true');
  $('#announcements').textContent = 'Searching for devices';
  try { await act('refresh'); await poll(); }
  catch (_) { /* act renders the failure; periodic refresh remains available. */ }
  finally { refreshing = false; $('#refresh').setAttribute('aria-busy', 'false'); $('#announcements').textContent = 'Search finished'; scheduleDiscovery(); }
}
async function preparePair(address) {
  try {
    pairing = await act('prepare_pair', {address});
    $('#pair-name').textContent = pairing.name;
    $('#pair-avatar').textContent = initials(pairing.name);
    $('#pair-code').textContent = pairing.code;
    $('#pair-status').textContent = '';
    $('#outgoing-pair').showModal();
    $('#pair-confirm').disabled = false;
  } catch (_) { /* act displays pairing errors. */ }
}
function deviceSignature(device) {
  return JSON.stringify([device.target,device.name,device.platform,device.paired,device.online,device.mismatch]);
}
function deviceTile(device) {
    const tile = element('div', undefined, 'device-tile');
    const row = element('button', undefined, 'device');
    row.dataset.target = device.target;
    row.disabled = !device.online || device.mismatch;
    row.title = device.mismatch ? 'Fingerprint mismatch. Verify this device locally.' : `${device.paired ? 'Connected to' : 'Pair with'} ${device.name}`;
    row.setAttribute('aria-label', `${device.name}, ${device.mismatch ? 'fingerprint mismatch' : device.paired ? 'Paired' : 'Unpaired'}, ${device.online ? 'Online' : 'Offline'}`);
    row.addEventListener('click', () => tapDevice(deviceItems.get(row.dataset.target)));
    const avatar = element('span', initials(device.name), 'avatar');
    const glyph = element('span', undefined, 'platform-glyph'); glyph.append(icon(/android|ios/i.test(device.platform) ? 'phone' : 'computer'));
    avatar.append(glyph);
    const badge = element('span', undefined, 'trust-glyph');
    badge.append(icon(device.mismatch ? 'warning' : device.paired ? 'check' : 'lock'));
    avatar.append(badge);
    const ring = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    ring.setAttribute('viewBox','0 0 100 100'); ring.setAttribute('class','progress-ring'); ring.setAttribute('aria-hidden','true');
    const circle = document.createElementNS(ring.namespaceURI, 'circle');
    for (const [key,value] of Object.entries({cx:50,cy:50,r:46,fill:'none','stroke-width':3,'pathLength':100,'stroke-dasharray':'0 100'})) circle.setAttribute(key,value);
    ring.append(circle); avatar.append(ring);
    row.append(avatar, element('span',device.name,'device-name'),element('span', device.paired ? 'Connected (Paired)' : 'Unpaired', 'connection-state'));
    tile.dataset.signature = deviceSignature(device);
    tile.append(row);
    return tile;
}
function devices(items) {
  // Refresh action data even when nonvisual discovery metadata alone changed.
  deviceItems = new Map(items.map(device => [device.target,device]));
  const signature = JSON.stringify(items.map(deviceSignature).sort());
  if (signature === lastDevices) return;
  lastDevices = signature;
  const grid = $('#devices');
  const activeTarget = document.activeElement?.dataset.target;
  for (const tile of grid.querySelectorAll('.device-tile')) {
    if (!deviceItems.has(tile.querySelector('.device').dataset.target)) tile.remove();
  }
  if (items.length) {
    grid.querySelectorAll(':scope > p, :scope > button').forEach(node => node.remove());
    for (const device of items) {
      const existing = [...grid.querySelectorAll('.device-tile')].find(tile => tile.querySelector('.device').dataset.target === device.target);
      if (!existing) grid.append(deviceTile(device));
      else if (existing.dataset.signature !== deviceSignature(device)) existing.replaceWith(deviceTile(device));
    }
  } else {
    grid.replaceChildren(element('p','No nearby devices yet.'));
    const add = element('button', 'Add Device', 'secondary'); add.prepend(icon('plus'));
    add.addEventListener('click', () => showPage(true)); grid.append(add);
  }
  updateAvatars();
  if (activeTarget) [...grid.querySelectorAll('.device')].find(node => node.dataset.target === activeTarget)?.focus();
}
function updateAvatars() {
  for (const row of document.querySelectorAll('.device')) {
    const transfer = [...currentTransfers].reverse().find(item => item.direction === 'send' && item.peer === row.dataset.target);
    if (!transfer) continue;
    const transferSignature = JSON.stringify(transfer);
    if (row.dataset.transfer === transferSignature) continue;
    row.dataset.transfer = transferSignature;
    const bytes = transfer.files.reduce((sum,file) => sum + file.bytes,0);
    const total = transfer.files.reduce((sum,file) => sum + file.total,0);
    const percent = transfer.status === 'complete' ? 100 : total ? Math.floor(100*bytes/total) : 0;
    const status = transfer.status;
    row.dataset.status = status;
    row.querySelector('circle').setAttribute('stroke-dasharray', `${percent} 100`);
    const label = `${row.querySelector('.device-name').textContent}, ${status}, ${percent}%`;
    row.setAttribute('aria-label',label); row.title = label;
    let badge = row.querySelector('.result-glyph');
    if (!badge) { badge = element('span',undefined,'result-glyph'); row.querySelector('.avatar').append(badge); }
    badge.replaceChildren(icon(status === 'complete' ? 'check' : status === 'rejected' || status === 'cancelled' ? 'close' : status === 'failed' || status === 'timed_out' ? 'warning' : 'clock'));
    const tile = row.parentNode;
    let cancel = tile.querySelector('.cancel-transfer');
    if (['waiting','accepted'].includes(status)) {
      if (!cancel) {
        cancel = iconButton('close',`Cancel send to ${row.querySelector('.device-name').textContent}`,'cancel-transfer');
        cancel.addEventListener('click', async () => { try { await act('cancel',{id:cancel.dataset.id}); await poll(); } catch (_) { /* rendered */ } });
        tile.append(cancel);
      }
      cancel.dataset.id = transfer.id;
    } else cancel?.remove();
  }
}
function invitations(items) {
  const signature = JSON.stringify(items);
  if (signature === lastInvitations) return;
  lastInvitations = signature;
  const container = $('#invitations');
  container.querySelectorAll('.invitation').forEach(row => { if (!items.some(item => item.id === row.dataset.id)) row.remove(); });
  for (const invitation of items) {
    const previous = [...container.children].find(row => row.dataset.id === invitation.id);
    const itemSignature = JSON.stringify(invitation);
    if (previous?.dataset.signature === itemSignature) continue;
    const box = element('div', undefined, 'invitation');
    box.dataset.id = invitation.id; box.dataset.signature = itemSignature;
    box.append(element('div', initials(invitation.name || invitation.peer), 'avatar'));
    box.append(element('h3', `${invitation.kind === 'pair' ? 'Pair with' : 'Receive from'} ${invitation.name || invitation.peer}`));
    if (invitation.kind === 'pair') box.append(element('strong', invitation.detail.code, 'code'));
    else for (const file of invitation.detail.files) box.append(element('p', `${file.name} · ${fileSize(file.size)}`));
    for (const accept of [false, true]) {
      const button = element('button', accept ? 'Accept' : 'Decline');
      button.dataset.action = accept ? 'accept' : 'reject';
      button.className = accept ? 'primary' : 'text-button';
      button.addEventListener('click', async () => {
        button.disabled = true;
        try { await act('decide', {id:invitation.id, accept}); await poll(); } catch (_) { button.disabled = false; }
      }); box.append(button);
    }
    if (previous) previous.replaceWith(box); else container.append(box);
  }
  const dialog = $('#incoming-dialog');
  if (items.length && !dialog.open) dialog.showModal();
  if (!items.length && dialog.open) dialog.close();
}
let lastTransfers = '';
function transfers(items) {
  const signature = JSON.stringify(items);
  if (signature === lastTransfers) return;
  lastTransfers = signature; currentTransfers = items; updateAvatars();
  const announcement = items.map(item => {
    const bytes = item.files.reduce((sum,file) => sum + file.bytes,0);
    const total = item.files.reduce((sum,file) => sum + file.total,0);
    const percent = item.status === 'complete' ? 100 : total ? Math.floor(10*bytes/total)*10 : 0;
    return `${item.peer}: ${item.status}, ${percent}%`;
  }).join('. ');
  if ($('#announcements').textContent !== announcement) $('#announcements').textContent = announcement;
  const container = $('#transfers');
  container.querySelectorAll('.transfer').forEach(row => { if (!items.some(item => item.id === row.dataset.id)) row.remove(); });
  for (const transfer of items) {
    const previous = [...container.children].find(row => row.dataset.id === transfer.id);
    const itemSignature = JSON.stringify(transfer);
    if (previous?.dataset.signature === itemSignature) continue;
    const box = element('div', undefined, 'transfer'); box.dataset.id = transfer.id; box.dataset.signature = itemSignature;
    const heading = element('h3', `${transfer.direction === 'send' ? 'Sent to' : 'Received from'} ${transfer.peer}`);
    heading.prepend(icon(transfer.direction === 'send' ? 'up' : 'down')); box.append(heading);
    const status = element('p', ({waiting:'Waiting for acceptance', accepted:'Transferring', complete:'Complete', rejected:'Rejected', timed_out:'Timed out', failed:'Failed', cancelled:'Cancelled'}[transfer.status] || transfer.status), 'transfer-status'); status.prepend(icon(transfer.status === 'complete' ? 'check' : ['rejected','cancelled'].includes(transfer.status) ? 'close' : ['failed','timed_out'].includes(transfer.status) ? 'warning' : 'clock')); box.append(status);
    for (const file of transfer.files) {
      const percent = file.status === 'complete' ? 100 : file.total ? Math.floor(100*file.bytes/file.total) : 0;
      box.append(element('p', `${file.name} · ${fileSize(file.total)} · ${percent}%`));
      if (file.saved_location) {
        box.append(element('p', `Saved to: ${file.saved_location}`, 'saved-location'));
      }
      if (file.saved_location && transfer.status === 'complete' && (platform !== 'android' || transfer.direction === 'receive')) {
        const reveal = iconButton(platform === 'android' ? 'share' : 'folder', platform === 'android' ? `Share ${file.name}` : `Reveal ${file.name}`);
        reveal.addEventListener('click', () => act(platform === 'android' ? 'share_file' : 'reveal_file', platform === 'android' ? {name:file.name} : {id:transfer.id,name:file.name}).catch(() => {}));
        box.append(reveal);
      }
      const bar = element('progress'); bar.max = 100; bar.value = percent; bar.setAttribute('aria-label', file.name); box.append(bar);
    }
    if (transfer.error) box.append(element('p', transfer.error));
    if (['waiting','accepted'].includes(transfer.status)) {
      const cancel = element('button','Cancel','secondary');
      cancel.addEventListener('click', async () => { try { await act('cancel',{id:transfer.id}); await poll(); } catch (_) { /* rendered */ } }); box.append(cancel);
    }
    if (previous) previous.replaceWith(box); else container.append(box);
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
function setText(selector, text) {
  if ($(selector).textContent !== text) $(selector).textContent = text;
}
async function updateSnapshot() {
  try {
    const state = await request('snapshot');
    const signature = JSON.stringify(state);
    if (state.platform === 'android') {
      if (!androidStarted) { androidStarted = true; configureAndroid(); startAndroid(); }
      if (canSearch()) checkSharedFiles();
    }
    if (signature === lastSnapshot) return;
    lastSnapshot = signature;
    platform = state.platform;
    setText('#my-name',state.name);
    setText('#fingerprint',state.fingerprint);
    setText('#qr-content',state.qr_content);
    if (!state.error && $('#error').hidden && !receivingStopped) setServiceState(`Receiving on port ${state.port}`);
    const qrSignature = JSON.stringify(state.qr);
    if (qrSignature !== lastQR) { lastQR = qrSignature; drawQR(state.qr); }
    devices(state.devices); invitations(state.invitations); transfers(state.transfers);
    chosenFiles = state.selected_files || [];
    renderFiles();
    if (state.error) failure(state.error);
  } catch (error) { failure(error); }
}
function poll() {
  // One in-flight snapshot prevents slow IPC from accumulating timer requests.
  if (!pollInFlight) pollInFlight = updateSnapshot().finally(() => { pollInFlight = null; });
  return pollInFlight;
}
function configureAndroid() {
  $('#android-controls').hidden = false;
  $('#scan-qr').hidden = false;
  $('#setting-folder').readOnly = true;
  $('#setting-folder').value = 'Download';
  document.querySelector('footer').textContent = 'Background reception uses the ongoing notification. Stop receiving to close the listener.';
  $('#open-folder').hidden = true;
  const tailscale = document.querySelector('[name=provider][value=tailscale]');
  tailscale.disabled = true; tailscale.parentNode.title = 'Desktop-only. Use manual VPN addresses on Android.';
}
$('#manual').addEventListener('submit', async event => {
  event.preventDefault();
  try { await act('add_device', {address:$('#address').value}); $('#address').value = ''; setText('#add-status','Added. Press ↻ to search for this device.'); } catch (_) { /* rendered */ }
});
$('#refresh').addEventListener('click', refresh);
function cancelPairing() {
  pairing = null; $('#outgoing-pair').close();
}
$('#pair-dismiss').addEventListener('click', cancelPairing);
$('#outgoing-pair').addEventListener('cancel', event => { event.preventDefault(); cancelPairing(); });
// Native modal dialogs trap focus. Escape declines every visible request.
$('#incoming-dialog').addEventListener('cancel', event => {
  event.preventDefault();
  document.querySelectorAll('#invitations [data-action=reject]').forEach(button => button.click());
});
$('#pair-confirm').addEventListener('click', async () => {
  $('#pair-confirm').disabled = true; $('#pair-status').textContent = 'Waiting for the other device to accept…';
  try {
    const confirmed = pairing;
    await act('confirm_pair', confirmed);
    if (pairing !== confirmed) return;
    $('#outgoing-pair').close(); pairing = null;
    await poll();
  } catch (_) { $('#pair-status').textContent = 'Pairing failed. Cancel to return.'; }
});


$('#pick-files').addEventListener('click', async () => { try { await act('pick_files'); await poll(); } catch (_) { /* rendered */ } });
async function tapDevice(device) {
  if (!device || !device.online || device.mismatch || pairing) return;
  try {
    if (!device.paired) { await preparePair(device.pairing_address || device.target); return; }
    if (!chosenFiles.length) {
      setText('#send-hint','Press + to choose files first.');
      return;
    }
    setText('#send-hint','');
    await act('send', {targets:[device.target]});
    await poll();
  } catch (_) { /* act renders the failure. */ }
}
async function checkSharedFiles() {
  if (checkingShares) return;
  checkingShares = true;
  try {
    const result = await request('shared_files');
    if (result.paths && result.paths.length) { showPage(false); await poll(); }
  } catch (error) { failure(error); }
  finally { checkingShares = false; }
}
function showPage(more) {
  $('#send-page').hidden = more;
  $('#more-page').hidden = !more;
  if (more) $('#settings-title').focus();
}
$('#show-send').addEventListener('click', () => { showPage(false); $('#show-more').focus(); });
$('#show-more').addEventListener('click', () => showPage(true));
$('#open-folder').addEventListener('click', async () => { try { await act('open_receive_directory'); } catch (_) { /* rendered */ } });

let settings = null;
function renderSettings(value) {
  settings = value.config;
  $('#setting-name').value = settings.name;
  $('#setting-folder').value = platform === 'android' || androidStarted ? 'Download' : settings.receive_directory;
  document.querySelectorAll('[name=provider]').forEach(node => { node.checked = settings.providers.includes(node.value); });
  $('#setting-notifications').checked = settings.notifications_enabled;
}
async function saveSettings(payload) {
  try { renderSettings(await act('save_settings',payload)); $('#settings-status').textContent = 'Saved. Restart to apply device, folder and network settings; notifications updated now.'; $('#name-status').textContent = 'Saved. Restart to apply the new name.'; }
  catch (_) { $('#settings-status').textContent = 'Settings were not saved. Review the error and try again.'; $('#name-status').textContent = 'Name was not saved. Review the error and try again.'; }
}
$('#settings-form').addEventListener('submit', event => {
  event.preventDefault();
  saveSettings({config:{...settings, receive_directory:platform === 'android' ? settings.receive_directory : $('#setting-folder').value, providers:[...document.querySelectorAll('[name=provider]:checked')].map(node => node.value), notifications_enabled:$('#setting-notifications').checked}});
});
$('#name-form').addEventListener('submit', event => { event.preventDefault(); saveSettings({config:{...settings,name:$('#setting-name').value}}); });
act('settings').then(renderSettings).catch(() => { /* act renders configuration errors */ });

async function startAndroid() {
  try {
    const result = await act('android_start');
    $('#android-permission').textContent = result.permission === 'granted'
      ? 'Background reception enabled. Incoming invitations require your acceptance.'
      : 'Notifications denied. Enable notifications in Android Settings to see incoming alerts; invitations still require acceptance in the app.';
  } catch (_) { $('#android-permission').textContent = 'Background reception could not start. Keep the app visible and review the error.'; }
}
$('#stop-receiver').addEventListener('click', async () => {
  try { await act('android_stop'); receivingStopped = true; setServiceState('Reception stopped. Reopen the app to receive files.', true); $('#android-permission').textContent = 'Reception stopped. Reopen the app to start again.'; }
  catch (_) { /* displayed */ }
});

$('#scan-qr').addEventListener('click', async () => {
  try {
    const permission = await window.__TAURI__.core.invoke('plugin:barcode-scanner|request_permissions');
    if (permission.camera !== 'granted') throw new Error('Camera permission denied. Enter a host:port or paste QR content manually.');
    const result = await window.__TAURI__.core.invoke('plugin:barcode-scanner|scan',{formats:['QR_CODE'],windowed:false});
    if (!result.content || !result.content.startsWith('meshdrop://')) throw new Error('This is not a meshdrop pairing QR. Enter an address manually.');
    await act('add_device',{address:result.content}); setText('#add-status','Added. Press ↻ to search for this device.');
    showPage(false);
  } catch (error) { failure(error); }
});

function initials(name) { return String(name || '?').trim().split(/\s+/).slice(0,2).map(word => word[0]).join('').toUpperCase(); }
function setServiceState(text, error = false) {
  const state = $('#service-state');
  if (state.textContent === text && state.classList.contains('warning') === error) return;
  state.textContent = text; state.title = text; state.setAttribute('aria-label',text); state.classList.toggle('warning',error);
}
function renderFiles() {
  const signature = JSON.stringify(chosenFiles);
  if (signature === lastFiles) return;
  lastFiles = signature;
  if (chosenFiles.length) setText('#send-hint','');
  $('#selected-files').replaceChildren();
  chosenFiles.forEach((name,index) => {
    const chip = element('span',undefined,'file-chip'); chip.append(element('span',name));
    const remove = iconButton('close',`Remove ${name}`);
    remove.addEventListener('click', async () => { try { await act('remove_file',{index}); await poll(); } catch (_) { /* rendered */ } });
    chip.append(remove); $('#selected-files').append(chip);
  });
}
function icon(name) {
  const paths = {
    refresh:'M20 7v5h-5 M20 12a8 8 0 1 0-2 5 M20 7l-3-3',
    settings:'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8 M10 2h4l1 3 3 1 3 4v4l-3 1-1 3-3 3h-4l-1-3-3-1-3-3v-4l3-1 1-3z',
    plus:'M12 5v14 M5 12h14',close:'M6 6l12 12 M6 18L18 6',
    right:'M9 5l7 7-7 7',left:'M15 5l-7 7 7 7',
    computer:'M3 4h18v13H3z M8 21h8 M12 17v4',phone:'M7 2h10v20H7z M11 18h2',
    folder:'M3 7V4h7l2 3h9v13H3z',lock:'M6 10h12v11H6z M8 10V6a4 4 0 0 1 8 0v4',
    warning:'M12 3l10 18H2z M12 9v5 M12 17v1',check:'M5 12l4 4L19 6',
    clock:'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18 M12 7v6l4 2',
    up:'M12 20V4 M6 10l6-6 6 6',down:'M12 4v16 M6 14l6 6 6-6',
    share:'M12 16V2 M7 7l5-5 5 5 M5 11H3v10h18V11h-2',network:'M12 3v7 M4 21v-7h16v7 M12 14v7 M8 3h8v6H8z',
    clip:'M9 14l7-7a3 3 0 0 1 4 4L10 21a5 5 0 0 1-7-7L14 3'
  };
  const svg = document.createElementNS('http://www.w3.org/2000/svg','svg');
  for (const [key,value] of Object.entries({viewBox:'0 0 24 24',fill:'none',stroke:'currentColor','stroke-width':1.8,'stroke-linecap':'round','stroke-linejoin':'round','aria-hidden':'true'})) svg.setAttribute(key,value);
  const path = document.createElementNS(svg.namespaceURI,'path'); path.setAttribute('d',paths[name] || paths.warning); svg.append(path); return svg;
}
function iconButton(glyph,label,className = '') {
  const button = element('button',undefined,`icon-button ${className}`); button.type = 'button'; button.title = label; button.setAttribute('aria-label',label); button.append(icon(glyph)); return button;
}
$('#refresh').append(icon('refresh')); $('#show-more').append(icon('settings')); $('#show-send').append(icon('left')); $('#pick-files').append(icon('plus'));
function fileSize(bytes) {
  if (bytes < 1000) return `${bytes} B`;
  const units = ['kB','MB','GB','TB'];
  let value = bytes / 1000, index = 0;
  while (value >= 1000 && index < units.length - 1) { value /= 1000; index++; }
  return `${Number(value.toFixed(1))} ${units[index]}`;
}
function updateForeground() {
  if (canSearch()) scheduleDiscovery();
  else { clearTimeout(discoveryTimer); discoveryTimer = null; }
}
document.addEventListener('visibilitychange', updateForeground);
window.addEventListener('blur', () => { foreground = false; updateForeground(); });
window.addEventListener('focus', () => { foreground = true; updateForeground(); poll(); });
poll(); refresh();
const pollingTimer = setInterval(poll,300);
