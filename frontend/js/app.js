(function () {
  'use strict';
  var root = document.getElementById('smartsecure-prototype');
  if (!root) return;
  var pageNames = { overview:'Tổng quan', monitor:'Giám sát Camera', ai:'Giám sát AI', iot:'Thiết bị IoT', alerts:'Cảnh báo an ninh', history:'Lịch sử sự kiện', cameras:'Quản lý Camera', users:'Quản lý người dùng', system:'Trạng thái hệ thống', settings:'Cài đặt hệ thống' };
  function toast(message) {
    var node = root.querySelector('#ss-toast');
    if (!node) return;
    node.textContent = message;
    node.classList.add('show');
    window.clearTimeout(window.smartSecureToastTimer);
    window.smartSecureToastTimer = window.setTimeout(function () { node.classList.remove('show'); }, 2400);
  }
  function showPage(page) {
    root.querySelectorAll('.view').forEach(function (view) { view.hidden = view.dataset.page !== page; });
    root.querySelectorAll('.nav button[data-view]').forEach(function (button) {
      var selected = button.dataset.view === page;
      button.classList.toggle('active', selected);
      if (selected) button.setAttribute('aria-current', 'page'); else button.removeAttribute('aria-current');
    });
    var crumb = root.querySelector('#ss-crumb');
    if (crumb) crumb.textContent = pageNames[page] || pageNames.overview;
    var sidebar = root.querySelector('#ss-sidebar');
    if (sidebar) sidebar.classList.remove('open');
  }
  function confirmDialog(message, success) {
    var backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML = '<div class="modal" role="dialog" aria-modal="true" aria-labelledby="ss-confirm-title"><div class="modal-head"><span id="ss-confirm-title">Xác nhận thao tác</span><button class="modal-close" aria-label="Đóng">×</button></div><p class="confirm-copy"></p><div class="modal-foot"><button class="btn" data-cancel>Hủy</button><button class="btn primary" data-yes>Xác nhận</button></div></div>';
    backdrop.querySelector('.confirm-copy').textContent = message;
    root.querySelector('#ss-modal-root').appendChild(backdrop);
    function close() { backdrop.remove(); }
    backdrop.querySelector('.modal-close').addEventListener('click', close);
    backdrop.querySelector('[data-cancel]').addEventListener('click', close);
    backdrop.querySelector('[data-yes]').addEventListener('click', function () { close(); toast(success || 'Đã xác nhận thao tác minh họa.'); });
    backdrop.addEventListener('click', function (event) { if (event.target === backdrop) close(); });
  }
  function openForm(type) {
    var schemas = {
      camera: { title:'Thông tin Camera', fields:['Tên Camera','Địa chỉ IP','RTSP URL','Khu vực lắp đặt'] },
      user: { title:'Thông tin người dùng', fields:['Họ và tên','Email','Vai trò','Trạng thái'] },
      device: { title:'Thông tin thiết bị', fields:['Mã thiết bị','Tên thiết bị','Loại thiết bị','Khu vực'] }
    };
    var schema = schemas[type];
    if (!schema) return;
    var backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    var fields = schema.fields.map(function (label, index) {
      var typeName = label === 'Email' ? 'email' : 'text';
      var placeholder = label === 'RTSP URL' ? 'rtsp://••••••••@192.168.1.x/…' : 'Nhập ' + label.toLowerCase();
      return '<div class="modal-field"><label for="ss-field-' + index + '">' + label + '</label><input id="ss-field-' + index + '" type="' + typeName + '" placeholder="' + placeholder + '"></div>';
    }).join('');
    backdrop.innerHTML = '<div class="modal" role="dialog" aria-modal="true"><div class="modal-head">' + schema.title + '<button class="modal-close" aria-label="Đóng">×</button></div><div class="modal-grid">' + fields + '</div><div class="modal-foot"><button class="btn" data-cancel>Hủy</button><button class="btn primary" data-save>Lưu thông tin</button></div></div>';
    root.querySelector('#ss-modal-root').appendChild(backdrop);
    function close() { backdrop.remove(); }
    backdrop.querySelector('.modal-close').addEventListener('click', close);
    backdrop.querySelector('[data-cancel]').addEventListener('click', close);
    backdrop.querySelector('[data-save]').addEventListener('click', function () { close(); toast('Đã lưu thay đổi minh họa.'); });
    backdrop.addEventListener('click', function (event) { if (event.target === backdrop) close(); });
  }
  root.querySelectorAll('.nav button[data-view]').forEach(function (button) { button.addEventListener('click', function () { showPage(button.dataset.view); }); });
  root.querySelectorAll('[data-view-link]').forEach(function (button) { button.addEventListener('click', function () { showPage(button.dataset.viewLink); }); });
  var menu = root.querySelector('#ss-menu');
  if (menu) menu.addEventListener('click', function () { root.querySelector('#ss-sidebar').classList.toggle('open'); });
  var bell = root.querySelector('#ss-bell');
  if (bell) bell.addEventListener('click', function () { showPage('alerts'); });
  root.querySelectorAll('[data-toast]').forEach(function (button) { button.addEventListener('click', function () { toast(button.dataset.toast); }); });
  root.querySelectorAll('[data-modal]').forEach(function (button) { button.addEventListener('click', function () { openForm(button.dataset.modal); }); });
  root.querySelectorAll('[data-confirm]').forEach(function (button) { button.addEventListener('click', function () { confirmDialog(button.dataset.confirm); }); });
  root.querySelectorAll('.filter-chip').forEach(function (button) { button.addEventListener('click', function () {
    root.querySelectorAll('.filter-chip').forEach(function (chip) { chip.classList.remove('active'); });
    button.classList.add('active');
  }); });
  root.querySelectorAll('[data-pref]').forEach(function (button) { button.addEventListener('click', function () { button.classList.toggle('on'); }); });
  root.querySelectorAll('.toggle[data-toggle]').forEach(function (button) { button.addEventListener('click', function () {
    confirmDialog('Bạn có muốn đổi trạng thái ' + button.dataset.toggle + '?', 'Đã xác nhận lệnh minh họa; thiết bị thật chưa được điều khiển.');
  }); });
  var search = root.querySelector('[data-camera-search]');
  var region = root.querySelector('[data-camera-region]');
  var status = root.querySelector('[data-camera-status]');
  function filterCameras() {
    var query = search ? search.value.trim().toLocaleLowerCase('vi') : '';
    var regionValue = region ? region.value : 'all';
    var statusValue = status ? status.value : 'all';
    root.querySelectorAll('.camera-card').forEach(function (card) {
      var matches = (!query || card.dataset.search.indexOf(query) >= 0) && (regionValue === 'all' || card.dataset.region === regionValue) && (statusValue === 'all' || card.dataset.status === statusValue);
      card.hidden = !matches;
    });
  }
  if (search) search.addEventListener('input', filterCameras);
  if (region) region.addEventListener('change', filterCameras);
  if (status) status.addEventListener('change', filterCameras);
  root.querySelectorAll('[data-camera-detail]').forEach(function (button) { button.addEventListener('click', function () {
    var backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML = '<div class="modal camera-detail-modal" role="dialog" aria-modal="true"><div class="modal-head"><span class="camera-detail-title"></span><button class="modal-close" aria-label="Đóng">×</button></div><div class="camera-detail-scene"><span class="scene-building"></span><span class="scene-ground"></span><span class="person-shape"></span><span class="bbox"></span><span class="bbox-label">Person · 96.8%</span></div><div class="settings-list"><div><span>Khu vực</span><strong>Cổng phía Đông</strong></div><div><span>Kết nối</span><strong>RTSP · Đã che địa chỉ</strong></div><div><span>Trạng thái</span><span class="status online">Online · Minh họa</span></div></div></div>';
    backdrop.querySelector('.camera-detail-title').textContent = button.dataset.cameraDetail;
    root.querySelector('#ss-modal-root').appendChild(backdrop);
    function close() { backdrop.remove(); }
    backdrop.querySelector('.modal-close').addEventListener('click', close);
    backdrop.addEventListener('click', function (event) { if (event.target === backdrop) close(); });
  }); });
  root.querySelectorAll('[data-fullscreen]').forEach(function (button) { button.addEventListener('click', function (event) {
    var scene = event.currentTarget.closest('.camera-scene');
    if (scene && scene.requestFullscreen) scene.requestFullscreen().catch(function () { toast('Trình duyệt không cho phép toàn màn hình.'); });
  }); });
  root.querySelectorAll('.pages button').forEach(function (button) { button.addEventListener('click', function () {
    var group = button.closest('.pages');
    group.querySelectorAll('button').forEach(function (item) { item.classList.remove('active'); });
    button.classList.add('active');
  }); });
})();
