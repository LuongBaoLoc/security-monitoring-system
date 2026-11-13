(function () {
  'use strict';
  var root = document.getElementById('smartsecure-prototype');
  if (!root) return;

  var pageNames = {
    overview: 'Tổng quan',
    monitor: 'Giám sát Camera',
    ai: 'Giám sát AI',
    iot: 'Thiết bị IoT',
    alerts: 'Cảnh báo an ninh',
    history: 'Lịch sử sự kiện',
    cameras: 'Quản lý Camera',
    users: 'Quản lý người dùng',
    system: 'Trạng thái hệ thống',
    settings: 'Cài đặt hệ thống'
  };

  function toast(message) {
    var node = root.querySelector('#ss-toast');
    if (!node) return;
    node.textContent = message;
    node.classList.add('show');
    window.clearTimeout(window.smartSecureToastTimer);
    window.smartSecureToastTimer = window.setTimeout(function () {
      node.classList.remove('show');
    }, 2600);
  }

  function showPage(page) {
    root.querySelectorAll('.view').forEach(function (view) {
      view.hidden = view.dataset.page !== page;
    });
    root.querySelectorAll('.nav button[data-view]').forEach(function (button) {
      var selected = button.dataset.view === page;
      button.classList.toggle('active', selected);
      if (selected) button.setAttribute('aria-current', 'page');
      else button.removeAttribute('aria-current');
    });
    var crumb = root.querySelector('#ss-crumb');
    if (crumb) crumb.textContent = pageNames[page] || pageNames.overview;
    var sidebar = root.querySelector('#ss-sidebar');
    if (sidebar) sidebar.classList.remove('open');
  }

  function confirmDialog(message, onConfirm) {
    var backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML =
      '<div class="modal" role="dialog" aria-modal="true" aria-labelledby="ss-confirm-title">' +
      '<div class="modal-head"><span id="ss-confirm-title">Xác nhận thao tác</span><button class="modal-close" aria-label="Đóng">×</button></div>' +
      '<p class="confirm-copy"></p>' +
      '<div class="modal-foot"><button class="btn" data-cancel>Hủy</button><button class="btn primary" data-yes>Xác nhận</button></div>' +
      '</div>';
    backdrop.querySelector('.confirm-copy').textContent = message;
    root.querySelector('#ss-modal-root').appendChild(backdrop);
    function close() {
      backdrop.remove();
    }
    backdrop.querySelector('.modal-close').addEventListener('click', close);
    backdrop.querySelector('[data-cancel]').addEventListener('click', close);
    backdrop.querySelector('[data-yes]').addEventListener('click', function () {
      close();
      if (typeof onConfirm === 'function') onConfirm();
    });
    backdrop.addEventListener('click', function (event) {
      if (event.target === backdrop) close();
    });
  }

  root.querySelectorAll('.nav button[data-view]').forEach(function (button) {
    button.addEventListener('click', function () {
      showPage(button.dataset.view);
    });
  });

  root.querySelectorAll('[data-view-link]').forEach(function (button) {
    button.addEventListener('click', function () {
      showPage(button.dataset.viewLink);
    });
  });

  var menu = root.querySelector('#ss-menu');
  if (menu) {
    menu.addEventListener('click', function () {
      root.querySelector('#ss-sidebar').classList.toggle('open');
    });
  }

  var bell = root.querySelector('#ss-bell');
  if (bell) {
    bell.addEventListener('click', function () {
      showPage('alerts');
    });
  }

  root.querySelectorAll('[data-pref]').forEach(function (button) {
    button.addEventListener('click', function () {
      button.classList.toggle('on');
    });
  });

  window.smartSecureToast = toast;
  window.smartSecureShowPage = showPage;
  window.smartSecureConfirm = confirmDialog;
})();
