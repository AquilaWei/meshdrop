'use strict';
const request = (operation, payload = null) => window.__TAURI__.core.invoke('desktop_request', {operation, payload});
const $ = selector => document.querySelector(selector);
let chosenFiles = [];
let checkingShares = false;
let pairing = null;
let sendQueue = [];
let sending = false;
let refreshing = false;
let androidStarted = false;
let receivingStopped = false;
let lastDevices = '';
let lastInvitations = '';
let platform = '';
let currentTransfers = [];
let settingsPage = null;
let lastFiles = '';
function element(tag, content, className) {
  const node = document.createElement(tag);
  if (content !== undefined) node.textContent = String(content);
  if (className) node.className = className;
  return node;
}
function failure(error) {
  $('#error').textContent = `${error}. Try again or review Settings › Advanced for details.`;
  setServiceState('Receiver needs attention. Review the error.', true);
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
  $('#refresh').setAttribute('aria-busy', 'true');
  $('#announcements').textContent = 'Searching for devices';
  try { await act('refresh'); await poll(); }
  catch (_) { /* act renders the failure; periodic refresh remains available. */ }
  finally { refreshing = false; $('#refresh').setAttribute('aria-busy', 'false'); $('#announcements').textContent = 'Search finished'; }
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
  } catch (_) { sendQueue = []; finishSending(); }
}
function devices(items) {
  const signature = JSON.stringify(items);
  if (signature === lastDevices) return;
  lastDevices = signature;
  const grid = $('#devices');
  const activeTarget = document.activeElement?.dataset.target;
  grid.replaceChildren();
  for (const device of items) {
    const tile = element('div', undefined, 'device-tile');
    const row = element('button', undefined, 'device');
    row.dataset.target = device.target;
    row.disabled = !device.online || device.mismatch;
    row.title = device.mismatch ? 'Fingerprint mismatch. Verify this device locally.' : `Send to ${device.name}`;
    row.setAttribute('aria-label', `${device.name}, ${device.mismatch ? 'fingerprint mismatch' : device.paired ? 'Paired' : 'Unpaired'}, ${device.online ? 'Online' : 'Offline'}`);
    row.addEventListener('click', () => tapDevice(device));
    const avatar = element('span', initials(device.name), 'avatar');
    const glyph = element('span', undefined, 'platform-glyph'); glyph.append(icon(/android|ios/i.test(device.platform) ? 'phone' : 'computer'));
    avatar.append(glyph);
    const badge = element('span', undefined, 'trust-glyph');
    if (!device.paired || device.mismatch) badge.append(icon(device.mismatch ? 'warning' : 'lock'));
    avatar.append(badge);
    const ring = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    ring.setAttribute('viewBox','0 0 100 100'); ring.setAttribute('class','progress-ring'); ring.setAttribute('aria-hidden','true');
    const circle = document.createElementNS(ring.namespaceURI, 'circle');
    for (const [key,value] of Object.entries({cx:50,cy:50,r:46,fill:'none','stroke-width':3,'pathLength':100,'stroke-dasharray':'0 100'})) circle.setAttribute(key,value);
    ring.append(circle); avatar.append(ring);
    row.append(avatar, element('span',device.name,'device-name'),element('span', device.paired ? 'Paired' : 'Unpaired', 'sr-only'));
    tile.append(row); grid.append(tile);
  }
  if (!items.length) {
    grid.append(element('p','No nearby devices yet.'));
    const add = element('button', 'Add Device', 'secondary'); add.prepend(icon('plus'));
    add.addEventListener('click', () => { showPage(true); openSettings('add'); }); grid.append(add);
  }
  updateAvatars();
  if (activeTarget) [...grid.querySelectorAll('.device')].find(node => node.dataset.target === activeTarget)?.focus();
}
function updateAvatars() {
  for (const row of document.querySelectorAll('.device')) {
    const transfer = [...currentTransfers].reverse().find(item => item.direction === 'send' && item.peer === row.dataset.target);
    if (!transfer) continue;
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
  $('#invitations').replaceChildren();
  for (const invitation of items) {
    const box = element('div', undefined, 'invitation');
    box.append(element('div', initials(invitation.name || invitation.peer), 'avatar'));
    box.append(element('h3', `${invitation.kind === 'pair' ? 'Pair with' : 'Receive from'} ${invitation.name || invitation.peer}`));
    if (invitation.kind === 'pair') box.append(element('strong', invitation.detail.code, 'code'));
    else for (const file of invitation.detail.files) box.append(element('p', `${file.name} · ${file.size} bytes`));
    for (const accept of [false, true]) {
      const button = element('button', accept ? 'Accept' : 'Decline');
      button.dataset.action = accept ? 'accept' : 'reject';
      button.className = accept ? 'primary' : 'text-button';
      button.addEventListener('click', async () => {
        button.disabled = true;
        try { await act('decide', {id:invitation.id, accept}); await poll(); } catch (_) { button.disabled = false; }
      }); box.append(button);
    }
    $('#invitations').append(box);
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
  $('#transfers').replaceChildren();
  for (const transfer of items) {
    const box = element('div', undefined, 'transfer'); box.dataset.id = transfer.id;
    const heading = element('h3', `${transfer.direction === 'send' ? 'Sent to' : 'Received from'} ${transfer.peer}`);
    heading.prepend(icon(transfer.direction === 'send' ? 'up' : 'down')); box.append(heading);
    const status = element('p', ({waiting:'Waiting for acceptance', accepted:'Transferring', complete:'Complete', rejected:'Rejected', timed_out:'Timed out', failed:'Failed', cancelled:'Cancelled'}[transfer.status] || transfer.status), 'transfer-status'); status.prepend(icon(transfer.status === 'complete' ? 'check' : ['rejected','cancelled'].includes(transfer.status) ? 'close' : ['failed','timed_out'].includes(transfer.status) ? 'warning' : 'clock')); box.append(status);
    for (const file of transfer.files) {
      const percent = file.status === 'complete' ? 100 : file.total ? Math.floor(100*file.bytes/file.total) : 0;
      box.append(element('p', `${file.name} · ${percent}%`));
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
    platform = state.platform;
    $('#my-name').textContent = state.name;
    if (state.platform === 'android') {
      $('#android-controls').hidden = settingsPage !== 'receiving';
      $('#scan-qr').hidden = settingsPage !== 'add';
      if (!androidStarted) { androidStarted = true; startAndroid(); }
      $('#setting-folder').readOnly = true;
      document.querySelector('footer').textContent = 'Background reception uses the ongoing notification. Stop it explicitly to close the listener.';
      $('#open-folder').disabled = true; $('#open-folder').title = 'Android 10+ saves verified files in Download; older Android uses private storage.';
      checkSharedFiles();
      const tailscale = document.querySelector('[name=provider][value=tailscale]');
      tailscale.disabled = true; tailscale.parentNode.title = 'Desktop-only. Use manual VPN addresses on Android.';
    }
    $('#fingerprint').textContent = state.fingerprint;
    $('#qr-content').textContent = state.qr_content;
    if (!state.error && $('#error').hidden && !receivingStopped) setServiceState(`Receiving on port ${state.port}`);
    drawQR(state.qr); devices(state.devices); invitations(state.invitations); transfers(state.transfers);
    chosenFiles = state.selected_files || [];
    renderFiles();
    $('#problems').replaceChildren(...state.problems.map(problem => element('li', `${problem.provider}: ${problem.error}`)));
    if (state.error) failure(state.error);
  } catch (error) { failure(error); }
}
$('#manual').addEventListener('submit', async event => {
  event.preventDefault();
  try { await act('add_device', {address:$('#address').value}); $('#address').value = ''; await refresh(); } catch (_) { /* rendered */ }
});
$('#refresh').addEventListener('click', refresh);
function finishSending() {
  sending = false;
}
function cancelPairing() {
  pairing = null; sendQueue = []; $('#outgoing-pair').close(); finishSending();
}
$('#pair-dismiss').addEventListener('click', cancelPairing);
$('#outgoing-pair').addEventListener('cancel', event => { event.preventDefault(); cancelPairing(); });
// Native modal dialogs trap focus. Escape declines every visible request.
$('#incoming-dialog').addEventListener('cancel', event => {
  event.preventDefault();
  document.querySelectorAll('#invitations [data-action=reject]').forEach(button => button.click());
});
async function nextTarget() {
  const device = sendQueue.shift();
  if (!device) { finishSending(); return; }
  if (!device.paired) { await preparePair(device.pairing_address || device.target); return; }
  try { await act('send', {targets:[device.target]}); }
  catch (_) { /* Keep independent targets moving after a command failure. */ }
  await poll(); await nextTarget();
}

$('#pair-confirm').addEventListener('click', async () => {
  $('#pair-confirm').disabled = true; $('#pair-status').textContent = 'Waiting for the other device to accept…';
  try {
    const confirmed = pairing;
    const target = confirmed.address;
    await act('confirm_pair', confirmed);
    if (pairing !== confirmed) return;
    $('#outgoing-pair').close(); pairing = null;
    await act('send', {targets:[target]});
    await refresh(); await nextTarget();
  } catch (_) { $('#pair-status').textContent = 'Pairing or sending failed. Cancel to return.'; sendQueue = []; finishSending(); }
});


$('#pick-files').addEventListener('click', async () => { try { await act('pick_files'); await poll(); } catch (_) { /* rendered */ } });
async function tapDevice(device) {
  try {
    if (!chosenFiles.length) {
      const files = await act('pick_files');
      if (!files || !files.length) return;
      await poll();
    }
    sendQueue.push(device);
    if (!sending) { sending = true; await nextTarget(); }
  } catch (_) { /* act renders errors; another device can still be tapped. */ }
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
const settingGroups = [ ['identity','computer','My Device'], ['add','plus','Add Device'], ['receiving','folder','Receiving'], ['network','network','Network'], ['advanced','settings','Advanced'] ];
function showPage(more) {
  $('#send-page').hidden = more;
  $('#more-page').hidden = !more;
  if (more) openSettings(null);
}
function openSettings(page) {
  settingsPage = page;
  $('#settings-list').hidden = Boolean(page);
  $('#settings-content').hidden = !page;
  $('#settings-title').textContent = settingGroups.find(group => group[0] === page)?.[2] || 'Settings';
  document.querySelectorAll('[data-page]').forEach(node => { node.hidden = node.dataset.page !== page; });
  $('#android-controls').hidden = platform !== 'android' || page !== 'receiving';
  $('#scan-qr').hidden = platform !== 'android' || page !== 'add';
  $('#configuration').hidden = !['receiving','network','advanced'].includes(page);
  $('#settings-form').hidden = page === 'advanced';
  $('#settings-title').focus();
}
$('#show-send').addEventListener('click', () => {
  if (settingsPage) openSettings(null);
  else { showPage(false); $('#show-more').focus(); }
});
$('#show-more').addEventListener('click', () => showPage(true));
for (const [page,glyph,title] of settingGroups) {
  const button = element('button',undefined,'settings-row'); button.dataset.settings = page;
  button.append(icon(glyph),element('span',title),icon('right'));
  button.addEventListener('click', () => openSettings(page)); $('#settings-list').append(button);
}
$('#open-folder').addEventListener('click', async () => { try { await act('open_receive_directory'); } catch (_) { /* rendered */ } });

let settings = null;
function renderSettings(value) {
  settings = value.config;
  $('#setting-name').value = settings.name;
  $('#setting-folder').value = settings.receive_directory; $('#setting-port').value = settings.port;
  $('#setting-networks').value = settings.allowed_networks.join('\n');
  document.querySelectorAll('[name=provider]').forEach(node => { node.checked = settings.providers.includes(node.value); });
  $('#setting-invitation').value = settings.invitation_timeout_seconds; $('#setting-idle').value = settings.idle_timeout_seconds;
  $('#setting-notifications').checked = settings.notifications_enabled;
  $('#settings-toml').value = value.toml;
}
async function saveSettings(payload) {
  try { renderSettings(await act('save_settings',payload)); $('#settings-status').textContent = 'Saved. Restart to apply device, folder and network settings; notifications updated now.'; $('#name-status').textContent = 'Saved. Restart to apply the new name.'; }
  catch (_) { $('#settings-status').textContent = 'Settings were not saved. Review the error and try again.'; $('#name-status').textContent = 'Name was not saved. Review the error and try again.'; }
}
$('#settings-form').addEventListener('submit', event => {
  event.preventDefault();
  saveSettings({config:{...settings, receive_directory:$('#setting-folder').value, port:Number($('#setting-port').value), allowed_networks:$('#setting-networks').value.split(/\s+/).filter(Boolean), providers:[...document.querySelectorAll('[name=provider]:checked')].map(node => node.value), invitation_timeout_seconds:Number($('#setting-invitation').value), idle_timeout_seconds:Number($('#setting-idle').value), notifications_enabled:$('#setting-notifications').checked}});
});
$('#name-form').addEventListener('submit', event => { event.preventDefault(); saveSettings({config:{...settings,name:$('#setting-name').value}}); });
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
  try { await act('android_stop'); receivingStopped = true; setServiceState('Reception stopped. Reopen the app to receive files.', true); $('#android-permission').textContent = 'Reception stopped. Reopen the app to start again.'; }
  catch (_) { /* displayed */ }
});

$('#scan-qr').addEventListener('click', async () => {
  try {
    const permission = await window.__TAURI__.core.invoke('plugin:barcode-scanner|request_permissions');
    if (permission.camera !== 'granted') throw new Error('Camera permission denied. Enter a host:port or paste QR content manually.');
    const result = await window.__TAURI__.core.invoke('plugin:barcode-scanner|scan',{formats:['QR_CODE'],windowed:false});
    if (!result.content || !result.content.startsWith('meshdrop://')) throw new Error('This is not a meshdrop pairing QR. Enter an address manually.');
    await act('add_device',{address:result.content}); await refresh();
    showPage(false);
  } catch (error) { failure(error); }
});

function initials(name) { return String(name || '?').trim().split(/\s+/).slice(0,2).map(word => word[0]).join('').toUpperCase(); }
function setServiceState(text, error = false) {
  const state = $('#service-state'); state.textContent = text; state.title = text; state.setAttribute('aria-label',text); state.classList.toggle('warning',error);
}
function renderFiles() {
  const signature = JSON.stringify(chosenFiles);
  if (signature === lastFiles) return;
  lastFiles = signature; $('#selected-files').replaceChildren();
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
$('#refresh').append(icon('refresh')); $('#show-more').append(icon('settings')); $('#show-send').append(icon('left')); $('#pick-files').append(icon('clip'));
// Listen to Tauri's native event: browser File objects do not expose safe paths.
window.__TAURI__.event?.listen('tauri://drag-drop', async event => {
  if (platform === 'android') return;
  try { await act('select_files',event.payload.paths); showPage(false); await poll(); } catch (_) { /* rendered */ }
}).catch(failure);
poll(); refresh();
const pollingTimer = setInterval(poll,300);
const discoveryTimer = setInterval(refresh,8000);
