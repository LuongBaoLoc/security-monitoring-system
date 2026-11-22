(function () {
  'use strict';
  var token = sessionStorage.getItem('smartsecure_token');
  var apiRoot = '/api';
  var activeSocket = null;

  var state = {
    me: null,
    overview: {},
    cameras: [],
    cameraLocations: [],
    monitorCameras: null,
    mgmtCameras: null,
    devices: [],
    selectedDeviceCode: null,
    alerts: [],
    detections: [],
    filteredDetections: null,
    detStats: [],
    alertStats: [],
    logs: [],
    users: [],
    health: {},
    settings: {},
    aiChipFilter: 'all'
  };
  var camFilterTimer = null;
  var aiFilterTimer = null;
  var wsReconnectTimer = null;

  function resolveUploadUrl(imagePath) {
    if (!imagePath || typeof imagePath !== 'string') return null;
    var clean = imagePath.trim().replace(/\\/g, '/').replace(/^\.\//, '');
    if (!clean || clean.indexOf('..') >= 0 || clean.indexOf(':') >= 0) return null;
    return clean.charAt(0) === '/' ? clean : ('/' + clean);
  }

  function requestMultipart(path, formData) {
    var headers = {};
    if (token) headers.Authorization = 'Bearer ' + token;
    return fetch(apiRoot + path, {
      method: 'POST',
      headers: headers,
      body: formData
    }).then(function (response) {
      if (!response.ok) {
        return response.json().catch(function () { return {}; }).then(function (body) {
          var err = new Error('API ' + response.status);
          err.status = response.status;
          err.detail = (body && body.detail) ? String(body.detail) : ('Lỗi HTTP ' + response.status);
          throw err;
        });
      }
      return response.json();
    });
  }

  function toast(msg) {
    if (window.smartSecureToast) window.smartSecureToast(msg);
  }

  function confirmAction(msg, fn) {
    if (window.smartSecureConfirm) window.smartSecureConfirm(msg, fn);
    else if (window.confirm(msg)) fn();
  }

  function request(path, options) {
    options = options || {};
    options.headers = Object.assign({ 'Content-Type': 'application/json' }, options.headers || {});
    if (token) options.headers.Authorization = 'Bearer ' + token;
    return fetch(apiRoot + path, options).then(function (response) {
      if (!response.ok) {
        return response.json().catch(function () { return {}; }).then(function (body) {
          var err = new Error('API ' + response.status);
          err.status = response.status;
          err.detail = (body && body.detail) ? (typeof body.detail === 'string' ? body.detail : JSON.stringify(body.detail)) : ('Lỗi HTTP ' + response.status);
          throw err;
        });
      }
      return response.status === 204 ? null : response.json();
    });
  }

  function initials(name) {
    var parts = String(name || 'AD').trim().split(/\s+/);
    if (parts.length >= 2) return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
    return String(name || 'AD').slice(0, 2).toUpperCase();
  }

  function isAdmin() {
    return state.me && state.me.role === 'ADMIN';
  }

  function applyUserRole(me) {
    state.me = me;
    var ini = initials(me.full_name || me.username);
    var roleText = me.role === 'ADMIN' ? 'Quản trị viên' : 'Nhân viên vận hành';
    var topName = document.getElementById('ss-top-name');
    var sideName = document.getElementById('ss-side-name');
    var topAvatar = document.getElementById('ss-top-avatar');
    var sideAvatar = document.getElementById('ss-side-avatar');
    var topRole = document.getElementById('ss-top-role');
    var sideRole = document.getElementById('ss-side-role');
    var greeting = document.getElementById('ss-overview-greeting');

    if (topName) topName.textContent = me.full_name;
    if (sideName) sideName.textContent = me.full_name;
    if (topAvatar) topAvatar.textContent = ini;
    if (sideAvatar) sideAvatar.textContent = ini;
    if (topRole) {
      topRole.textContent = me.role;
      topRole.className = 'status ' + (me.role === 'ADMIN' ? 'info' : 'online');
    }
    if (sideRole) sideRole.textContent = roleText + ' (' + me.role + ')';
    if (greeting) greeting.textContent = 'Xin chào, ' + me.full_name + ' (' + roleText + '). Đây là tình hình giám sát PCCC hôm nay.';

    document.querySelectorAll('[data-admin-only]').forEach(function (el) {
      var hide = me.role !== 'ADMIN';
      el.hidden = hide;
      el.style.display = hide ? 'none' : '';
    });
    var navUsers = document.getElementById('ss-nav-users');
    if (navUsers) {
      var hideUsersNav = me.role !== 'ADMIN';
      navUsers.hidden = hideUsersNav;
      navUsers.style.display = hideUsersNav ? 'none' : '';
      var usersPage = document.querySelector('[data-page="users"]');
      if (hideUsersNav && usersPage && !usersPage.hidden && window.smartSecureShowPage) {
        window.smartSecureShowPage('overview');
      }
    }
    ['ss-set-mq2', 'ss-set-conf', 'ss-set-dedup', 'ss-set-retention'].forEach(function (id) {
      var input = document.getElementById(id);
      if (input) input.disabled = me.role !== 'ADMIN';
    });
  }

  function logout() {
    sessionStorage.removeItem('smartsecure_token');
    token = null;
    state.me = null;
    if (wsReconnectTimer) {
      clearTimeout(wsReconnectTimer);
      wsReconnectTimer = null;
    }
    if (activeSocket) {
      activeSocket.close();
      activeSocket = null;
    }
    loginDialog();
  }

  function loginDialog() {
    if (document.getElementById('ss-login')) return;
    var box = document.createElement('div');
    box.id = 'ss-login';
    box.innerHTML =
      '<div class="ss-login-card">' +
      '<div class="brand"><div class="brandmark">✓</div><div>SmartSecure<small>ĐĂNG NHẬP HỆ THỐNG GIÁM SÁT &amp; BÁO CHÁY PCCC</small></div></div>' +
      '<form novalidate>' +
      '<label>Tên đăng nhập<input name="username" autocomplete="username" required placeholder="Nhập tên đăng nhập (vd: admin)"></label>' +
      '<label>Mật khẩu<div class="password-wrap"><input name="password" type="password" autocomplete="current-password" required placeholder="Nhập mật khẩu"><button type="button" class="password-toggle" aria-label="Hiện/ẩn mật khẩu">Hiện</button></div></label>' +
      '<p role="alert"></p>' +
      '<button class="btn primary" type="submit">Đăng nhập</button>' +
      '<small>Hỗ trợ xác thực JWT & phân quyền Quản trị viên (ADMIN) / Nhân viên vận hành (OPERATOR).</small>' +
      '</form></div>';
    document.body.appendChild(box);
    var userInput = box.querySelector('input[name="username"]');
    var pwdInput = box.querySelector('input[name="password"]');
    var togglePwd = box.querySelector('.password-toggle');
    var submitBtn = box.querySelector('button[type="submit"]');
    if (userInput) userInput.focus();
    if (togglePwd && pwdInput) {
      togglePwd.addEventListener('click', function () {
        var showing = pwdInput.type === 'text';
        pwdInput.type = showing ? 'password' : 'text';
        togglePwd.textContent = showing ? 'Hiện' : 'Ẩn';
      });
    }
    box.querySelector('form').addEventListener('submit', function (event) {
      event.preventDefault();
      var alertEl = box.querySelector('[role=alert]');
      alertEl.textContent = '';
      var username = String(userInput.value || '').trim();
      var password = String(pwdInput.value || '');
      if (!username || !password) {
        alertEl.textContent = 'Vui lòng nhập đầy đủ tên đăng nhập và mật khẩu.';
        return;
      }
      submitBtn.disabled = true;
      submitBtn.textContent = 'Đang xác thực...';
      request('/auth/login', {
        method: 'POST',
        body: JSON.stringify({ username: username, password: password })
      }).then(function (data) {
        token = data.access_token;
        sessionStorage.setItem('smartsecure_token', token);
        if (data.user) applyUserRole(data.user);
        box.remove();
        toast('Đăng nhập thành công.');
        refresh();
      }).catch(function (err) {
        submitBtn.disabled = false;
        submitBtn.textContent = 'Đăng nhập';
        alertEl.textContent = err.detail || 'Không thể đăng nhập. Kiểm tra thông tin và kết nối máy chủ.';
      });
    });
  }

  function formatSeverityBadge(severity) {
    var sev = String(severity || 'INFO').toUpperCase();
    var cls = sev === 'CRITICAL' ? 'crit' : (sev === 'WARNING' ? 'warn' : 'info');
    var span = document.createElement('span');
    span.className = 'severity ' + cls;
    span.textContent = '● ' + sev;
    return span;
  }

  function formatAlertStatusBadge(status) {
    var st = String(status || 'OPEN').toUpperCase();
    var span = document.createElement('span');
    if (st === 'RESOLVED') {
      span.className = 'status resolved';
      span.textContent = 'Đã xử lý';
    } else {
      span.className = 'status critical';
      span.textContent = 'Chưa xử lý';
    }
    return span;
  }

  function emptyRow(colspan, message) {
    var tr = document.createElement('tr');
    var td = document.createElement('td');
    td.colSpan = colspan;
    td.style.textAlign = 'center';
    td.style.color = '#64748b';
    td.style.padding = '20px';
    td.textContent = message;
    tr.appendChild(td);
    return tr;
  }

  /* ==================== 1. OVERVIEW ==================== */
  function renderOverviewStats(s) {
    var overview = document.querySelector('[data-page="overview"]');
    if (!overview) return;
    var statCards = overview.querySelectorAll('.stats .stat');
    var values = [
      s.camera_total || 0,
      (s.camera_online || 0) + ' / ' + (s.camera_total || 0),
      s.device_total || 0,
      s.detections_today || 0,
      s.alerts_today || 0,
      s.critical_open || 0
    ];
    values.forEach(function (val, idx) {
      if (!statCards[idx]) return;
      var valNode = statCards[idx].querySelector('.stat-value');
      if (valNode) valNode.textContent = val;
    });

    var camOffline = Math.max(0, (s.camera_total || 0) - (s.camera_online || 0));
    var devOffline = Math.max(0, (s.device_total || 0) - (s.device_online || 0));
    var aiActive = (state.health && (state.health.ai_worker === 'online' || state.health.ai === 'online')) || (s.detections_today || 0) > 0;
    var footers = [
      (s.camera_online || 0) > 0
        ? ('<span class="trend"><span class="dot"></span>' + s.camera_online + ' đang hoạt động</span> · ' + camOffline + ' ngoại tuyến')
        : '<span style="color:#64748b"><span class="dot offline"></span>Tạm thời chưa hoạt động</span>',
      (s.camera_online || 0) > 0
        ? '<span class="trend"><span class="dot"></span>Đang hoạt động</span> · Luồng trực tuyến'
        : '<span style="color:#64748b"><span class="dot offline"></span>Tạm thời chưa hoạt động</span>',
      (s.device_online || 0) > 0
        ? ('<span class="trend"><span class="dot"></span>' + s.device_online + ' đang hoạt động</span> · ' + devOffline + ' ngoại tuyến')
        : '<span style="color:#64748b"><span class="dot offline"></span>Tạm thời chưa hoạt động</span>',
      aiActive
        ? '<span class="trend"><span class="dot"></span>Đang hoạt động</span> · Ghi nhận từ AI'
        : '<span style="color:#64748b"><span class="dot offline"></span>Tạm thời chưa hoạt động</span>',
      '<span class="trend down">' + (s.alerts_open || 0) + ' đang chờ</span> xử lý',
      (s.critical_open || 0) + ' cảnh báo mức CRITICAL đang mở'
    ];
    footers.forEach(function (html, idx) {
      if (!statCards[idx]) return;
      var foot = statCards[idx].querySelector('.stat-foot');
      if (foot) foot.innerHTML = html;
    });

    var navCount = document.querySelector('.nav button[data-view="alerts"] .nav-count');
    if (navCount) navCount.textContent = s.alerts_open || 0;

    var updatedNode = overview.querySelector('.head-actions .updated');
    if (updatedNode) updatedNode.textContent = 'Cập nhật lúc ' + new Date().toLocaleTimeString('vi-VN');
  }

  function renderOverviewBars() {
    var barsContainer = document.getElementById('ss-overview-bars');
    if (!barsContainer) return;
    var daysSelect = document.getElementById('ss-overview-days');
    var days = daysSelect ? parseInt(daysSelect.value, 10) || 7 : 7;
    var note = document.getElementById('ss-chart-range-note');
    if (note) note.textContent = 'Tổng sự kiện và cảnh báo trong ' + days + ' ngày gần nhất';

    var bucketCount = 7;
    var now = new Date();
    var buckets = [];
    for (var i = bucketCount - 1; i >= 0; i--) {
      var d = new Date(now.getTime() - i * 86400000);
      var key = d.toISOString().slice(0, 10);
      var label = d.toLocaleDateString('vi-VN', { day: '2-digit', month: '2-digit' });
      buckets.push({ key: key, label: label, ai: 0, alerts: 0 });
    }
    state.detections.forEach(function (det) {
      var k = String(det.timestamp || '').slice(0, 10);
      buckets.forEach(function (b) { if (b.key === k) b.ai++; });
    });
    state.alerts.forEach(function (al) {
      var k = String(al.created_at || '').slice(0, 10);
      buckets.forEach(function (b) { if (b.key === k) b.alerts++; });
    });
    var maxVal = 5;
    buckets.forEach(function (b) {
      if (b.ai > maxVal) maxVal = b.ai;
      if (b.alerts > maxVal) maxVal = b.alerts;
    });

    barsContainer.replaceChildren();
    buckets.forEach(function (b) {
      var aiPct = b.ai > 0 ? Math.max(8, Math.round((b.ai / maxVal) * 92)) : 2;
      var alPct = b.alerts > 0 ? Math.max(8, Math.round((b.alerts / maxVal) * 92)) : 2;
      var grp = document.createElement('div');
      grp.className = 'bar-group';
      grp.title = b.label + ': ' + b.ai + ' sự kiện AI, ' + b.alerts + ' cảnh báo';
      grp.innerHTML = '<i class="bar" style="height:' + aiPct + '%"></i><i class="bar second" style="height:' + alPct + '%"></i><small class="bar-label">' + b.label + '</small>';
      barsContainer.appendChild(grp);
    });
  }

  function renderOverviewAlerts() {
    var body = document.querySelector('[data-page="overview"] .table tbody');
    if (!body) return;
    body.replaceChildren();
    var items = state.alerts.slice(0, 5);
    if (!items.length) {
      body.appendChild(emptyRow(6, 'Chưa có cảnh báo nào.'));
      return;
    }
    items.forEach(function (alert) {
      var tr = document.createElement('tr');
      var tdId = document.createElement('td');
      var strong = document.createElement('strong');
      strong.textContent = '#ALT-' + String(alert.id).padStart(4, '0');
      tdId.appendChild(strong);

      var tdDesc = document.createElement('td');
      tdDesc.textContent = alert.description;

      var tdType = document.createElement('td');
      tdType.textContent = alert.alert_type;

      var tdSev = document.createElement('td');
      tdSev.appendChild(formatSeverityBadge(alert.severity));

      var tdTime = document.createElement('td');
      tdTime.textContent = new Date(alert.created_at).toLocaleTimeString('vi-VN');

      var tdStatus = document.createElement('td');
      tdStatus.appendChild(formatAlertStatusBadge(alert.status));

      [tdId, tdDesc, tdType, tdSev, tdTime, tdStatus].forEach(function (td) { tr.appendChild(td); });
      body.appendChild(tr);
    });
  }

  function renderOverviewDetections() {
    var overview = document.querySelector('[data-page="overview"]');
    if (!overview) return;
    var items = state.detStats || [];
    var total = items.reduce(function (acc, item) { return acc + (item.count || 0); }, 0);

    var center = overview.querySelector('.donut-center');
    if (center) center.innerHTML = total + '<small>sự kiện</small>';
    var aiNote = document.getElementById('ss-overview-ai-note');
    if (aiNote) aiNote.textContent = total + ' lượt phát hiện Lửa/Khói';
    var donut = document.getElementById('ss-overview-donut');

    var legendList = overview.querySelector('.donut-wrap .legend-list');
    if (!legendList) return;
    legendList.replaceChildren();
    if (total <= 0) {
      if (donut) donut.style.background = '#e2e8f0';
      var empty = document.createElement('div');
      empty.className = 'legend-row';
      empty.textContent = 'Chưa có dữ liệu nhận diện Lửa/Khói';
      legendList.appendChild(empty);
      return;
    }
    var colorByType = { fire: '#ef4444', flame: '#dc2626', smoke: '#f59e0b', gas_mq2: '#8b5cf6' };
    var fallbackColors = ['#ef4444', '#f59e0b', '#3b82f6', '#8b5cf6'];
    var stops = [];
    var currentDeg = 0;
    var labelMap = { fire: 'Lửa (Fire)', flame: 'Tia lửa (Flame)', smoke: 'Khói (Smoke)', gas_mq2: 'Khí gas MQ-2', other: 'Khác' };
    items.slice(0, 4).forEach(function (item, idx) {
      var pct = Math.round((item.count / total) * 100);
      var nextDeg = idx === items.slice(0, 4).length - 1 ? 100 : (currentDeg + pct);
      var key = String(item.object_type || '').toLowerCase();
      var color = colorByType[key] || fallbackColors[idx % fallbackColors.length];
      stops.push(color + ' ' + currentDeg + '% ' + nextDeg + '%');
      currentDeg = nextDeg;

      var row = document.createElement('div');
      row.className = 'legend-row';
      var dot = document.createElement('i');
      dot.style.background = color;
      var label = document.createTextNode(labelMap[key] || item.object_type);
      var strong = document.createElement('strong');
      strong.textContent = pct + '% (' + item.count + ')';
      row.appendChild(dot);
      row.appendChild(label);
      row.appendChild(strong);
      legendList.appendChild(row);
    });
    if (donut && stops.length) {
      donut.style.background = 'conic-gradient(' + stops.join(', ') + ')';
    }
  }

  function renderOverviewDevices() {
    var list = document.querySelector('[data-page="overview"] .device-list');
    if (!list) return;
    list.replaceChildren();
    var items = [];
    state.cameras.slice(0, 3).forEach(function (cam) {
      items.push({
        title: cam.name,
        sub: (cam.location || 'Camera') + (cam.ip_address ? ' · ' + cam.ip_address : ''),
        online: cam.status === 'ONLINE'
      });
    });
    state.devices.slice(0, 3).forEach(function (dev) {
      items.push({
        title: dev.device_name,
        sub: dev.device_code + (dev.latest_mq2 != null ? ' · MQ-2: ' + dev.latest_mq2 + ' ppm' : ''),
        online: dev.status === 'ONLINE'
      });
    });
    if (!items.length) {
      var empty = document.createElement('div');
      empty.style.color = '#64748b';
      empty.style.padding = '12px 0';
      empty.innerHTML = '<span class="dot offline"></span>Tạm thời chưa hoạt động (Chưa có thiết bị kết nối)';
      list.appendChild(empty);
      return;
    }
    items.forEach(function (item) {
      var row = document.createElement('div');
      row.className = 'device-row';
      var copy = document.createElement('div');
      copy.className = 'device-copy';
      var strong = document.createElement('strong');
      strong.textContent = item.title;
      var small = document.createElement('small');
      small.textContent = item.sub;
      copy.appendChild(strong);
      copy.appendChild(small);

      var badge = document.createElement('span');
      badge.className = 'status ' + (item.online ? 'online' : 'unknown');
      badge.innerHTML = item.online
        ? '<span class="dot"></span>Đang hoạt động'
        : '<span class="dot offline"></span>Tạm thời chưa hoạt động';

      row.appendChild(copy);
      row.appendChild(badge);
      list.appendChild(row);
    });
  }

  function renderOverviewActivity() {
    var container = document.querySelector('[data-page="overview"] .activity');
    if (!container) return;
    container.replaceChildren();
    var items = state.logs.slice(0, 5);
    if (!items.length) {
      var empty = document.createElement('div');
      empty.style.color = '#64748b';
      empty.style.padding = '12px 0';
      empty.textContent = 'Chưa có hoạt động nào được ghi nhận.';
      container.appendChild(empty);
      return;
    }
    items.forEach(function (log) {
      var item = document.createElement('div');
      item.className = 'activity-item';
      var mark = document.createElement('i');
      mark.className = 'activity-mark' + (log.action === 'ALERT_RESOLVED' ? ' green' : (log.action === 'SENSOR_TELEMETRY' ? ' amber' : ''));
      var copy = document.createElement('div');
      copy.className = 'activity-copy';
      copy.textContent = log.description;
      var time = document.createElement('span');
      time.className = 'activity-time';
      time.textContent = new Date(log.created_at).toLocaleTimeString('vi-VN', { hour: '2-digit', minute: '2-digit' });
      item.appendChild(mark);
      item.appendChild(copy);
      item.appendChild(time);
      container.appendChild(item);
    });
  }

  function renderQuickCameras() {
    var grid = document.querySelector('[data-page="overview"] .quick-cam-grid');
    if (!grid) return;
    grid.replaceChildren();
    if (!state.cameras.length) {
      var empty = document.createElement('div');
      empty.style.color = '#64748b';
      empty.style.padding = '12px 0';
      empty.innerHTML = '<span class="dot offline"></span>Tạm thời chưa hoạt động (Chưa có camera kết nối)';
      grid.appendChild(empty);
      return;
    }
    state.cameras.slice(0, 3).forEach(function (cam) {
      var isOnline = cam.status === 'ONLINE';
      var card = document.createElement('button');
      card.className = 'quick-cam-card';
      card.innerHTML = isOnline
        ? '<span class="quick-preview"><b class="live-tag">● LIVE RTSP</b></span><span class="quick-copy"><strong></strong><small></small></span>'
        : '<span class="quick-preview scene-offline"><span class="offline-copy"><span class="dot offline"></span>Tạm thời chưa hoạt động</span><b class="live-tag offline-tag">OFFLINE</b></span><span class="quick-copy"><strong></strong><small></small></span>';
      card.querySelector('strong').textContent = cam.name;
      card.querySelector('small').textContent = (cam.location || 'Chưa rõ vị trí') + ' · ' + (isOnline ? 'Đang hoạt động' : 'Tạm thời chưa hoạt động');
      card.addEventListener('click', function () {
        if (window.smartSecureShowPage) window.smartSecureShowPage('monitor');
      });
      grid.appendChild(card);
    });
  }

  /* ==================== 2 & 7. CAMERAS (MONITOR & MANAGEMENT) ==================== */
  function openCameraDetailModal(cam) {
    var isOnline = cam.status === 'ONLINE';
    var camId = cam.camera_id != null ? cam.camera_id : cam.id;
    var backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML =
      '<div class="modal camera-detail-modal" role="dialog" aria-modal="true">' +
      '<div class="modal-head"><span class="camera-detail-title"></span><button class="modal-close" aria-label="Đóng">×</button></div>' +
      (isOnline
        ? '<div class="camera-detail-scene"><span class="scene-building"></span><span class="scene-ground"></span><span class="bbox-label">LIVE RTSP · PCCC</span></div>'
        : '<div class="camera-detail-scene scene-offline"><span class="offline-copy"><span class="dot offline"></span>Tạm thời chưa hoạt động (Camera đang Offline)</span></div>') +
      '<div class="settings-list">' +
      '<div><span>Mã Camera (camera_id)</span><span class="cam-id-badge" data-cam-id></span></div>' +
      '<div><span>Tên Camera</span><strong data-cam-name></strong></div>' +
      '<div><span>Khu vực lắp đặt</span><strong data-cam-loc></strong></div>' +
      '<div><span>Địa chỉ IP</span><strong data-cam-ip></strong></div>' +
      '<div><span>Luồng RTSP (đã che mật khẩu)</span><strong data-cam-rtsp style="font-family:var(--ss-font-mono);font-size:12px"></strong></div>' +
      '<div><span>Trạng thái thực tế</span><span class="status ' + (isOnline ? 'online' : 'unknown') + '"><span class="dot ' + (isOnline ? '' : 'offline') + '"></span>' + (isOnline ? 'Đang hoạt động' : 'Tạm thời chưa hoạt động') + '</span></div>' +
      '</div></div>';
    backdrop.querySelector('.camera-detail-title').textContent = cam.name + ' (ID #' + camId + ')';
    backdrop.querySelector('[data-cam-id]').textContent = 'camera_id: #' + camId;
    backdrop.querySelector('[data-cam-name]').textContent = cam.name;
    backdrop.querySelector('[data-cam-loc]').textContent = cam.location || 'Chưa đặt vị trí';
    backdrop.querySelector('[data-cam-ip]').textContent = cam.ip_address || '—';
    backdrop.querySelector('[data-cam-rtsp]').textContent = (cam.rtsp_url || '—') + (cam.rtsp_has_credentials ? ' (Đã bảo vệ mật khẩu)' : '');
    document.getElementById('ss-modal-root').appendChild(backdrop);
    function close() { backdrop.remove(); }
    backdrop.querySelector('.modal-close').addEventListener('click', close);
    backdrop.addEventListener('click', function (e) { if (e.target === backdrop) close(); });
  }

  function populateLocationSelect(selectEl, locations) {
    if (!selectEl) return;
    var prevVal = selectEl.value || 'all';
    selectEl.replaceChildren();
    var allOpt = document.createElement('option');
    allOpt.value = 'all';
    allOpt.textContent = 'Tất cả khu vực';
    selectEl.appendChild(allOpt);
    locations.forEach(function (loc) {
      var opt = document.createElement('option');
      opt.value = loc;
      opt.textContent = loc;
      selectEl.appendChild(opt);
    });
    if (prevVal !== 'all' && locations.indexOf(prevVal) >= 0) {
      selectEl.value = prevVal;
    }
  }

  function buildCameraQueryString(search, location, status) {
    var qs = ['limit=100'];
    if (search) qs.push('search=' + encodeURIComponent(search));
    if (location && location !== 'all') qs.push('location=' + encodeURIComponent(location));
    if (status && status !== 'all') qs.push('status=' + encodeURIComponent(status.toUpperCase()));
    return '/cameras?' + qs.join('&');
  }

  function fetchFilteredCameras(scope) {
    if (!token) return;
    if (scope === 'monitor') {
      var searchInput = document.querySelector('[data-camera-search]');
      var regionSelect = document.querySelector('[data-camera-region]');
      var statusSelect = document.querySelector('[data-camera-status]');
      var path = buildCameraQueryString(
        searchInput ? searchInput.value.trim() : '',
        regionSelect ? regionSelect.value : 'all',
        statusSelect ? statusSelect.value : 'all'
      );
      request(path).then(function (res) {
        state.monitorCameras = (res && res.items) || [];
        if (res && Array.isArray(res.locations)) state.cameraLocations = res.locations;
        renderCamerasViews();
      }).catch(function () {});
    } else if (scope === 'mgmt') {
      var mgmtSearch = document.getElementById('ss-cam-mgmt-search');
      var mgmtLoc = document.getElementById('ss-cam-mgmt-location');
      var mgmtStatus = document.getElementById('ss-cam-mgmt-status');
      var mPath = buildCameraQueryString(
        mgmtSearch ? mgmtSearch.value.trim() : '',
        mgmtLoc ? mgmtLoc.value : 'all',
        mgmtStatus ? mgmtStatus.value : 'all'
      );
      request(mPath).then(function (res) {
        state.mgmtCameras = (res && res.items) || [];
        if (res && Array.isArray(res.locations)) state.cameraLocations = res.locations;
        renderCamerasViews();
      }).catch(function () {});
    }
  }

  function renderCamerasViews() {
    var items = state.cameras;
    var locations = state.cameraLocations && state.cameraLocations.length
      ? state.cameraLocations
      : items.reduce(function (acc, c) {
          if (c.location && acc.indexOf(c.location) < 0) acc.push(c.location);
          return acc;
        }, []);

    var regionSelect = document.querySelector('[data-camera-region]');
    var mgmtLocSelect = document.getElementById('ss-cam-mgmt-location');
    populateLocationSelect(regionSelect, locations);
    populateLocationSelect(mgmtLocSelect, locations);

    var searchInput = document.querySelector('[data-camera-search]');
    var statusSelect = document.querySelector('[data-camera-status]');
    var q = searchInput ? searchInput.value.trim().toLowerCase() : '';
    var reg = regionSelect ? regionSelect.value : 'all';
    var st = statusSelect ? statusSelect.value : 'all';
    var hasMonitorFilter = Boolean(q || reg !== 'all' || st !== 'all');

    var gallery = document.querySelector('[data-page="monitor"] .camera-gallery');
    var monitorPill = document.getElementById('ss-monitor-head-pill');
    var onlineCount = items.filter(function (c) { return c.status === 'ONLINE'; }).length;
    if (monitorPill) {
      if (onlineCount > 0) {
        monitorPill.className = 'status online';
        monitorPill.innerHTML = '<i class="dot"></i>Đang hoạt động (' + onlineCount + ' / ' + items.length + ' trực tuyến)';
      } else {
        monitorPill.className = 'status unknown';
        monitorPill.innerHTML = '<i class="dot offline"></i>Tạm thời chưa hoạt động (0 / ' + items.length + ' trực tuyến)';
      }
    }
    if (gallery) {
      gallery.replaceChildren();
      var filteredMonitor = (hasMonitorFilter && Array.isArray(state.monitorCameras))
        ? state.monitorCameras
        : items.filter(function (cam) {
            var matchQ = !q || (cam.name + ' ' + (cam.location || '') + ' ' + (cam.ip_address || '')).toLowerCase().indexOf(q) >= 0;
            var matchReg = reg === 'all' || (cam.location || '').toLowerCase() === reg.toLowerCase();
            var matchSt = st === 'all' || (st.toLowerCase() === 'online' ? cam.status === 'ONLINE' : cam.status !== 'ONLINE');
            return matchQ && matchReg && matchSt;
          });
      if (!filteredMonitor.length) {
        var empty = document.createElement('div');
        empty.style.color = '#64748b';
        empty.style.padding = '24px';
        empty.innerHTML = '<span class="dot offline"></span>' + (items.length ? 'Không tìm thấy camera khớp bộ lọc.' : 'Tạm thời chưa hoạt động — Chưa có camera nào được kết nối.');
        gallery.appendChild(empty);
      } else {
        filteredMonitor.forEach(function (cam) {
          var isOnline = cam.status === 'ONLINE';
          var camId = cam.camera_id != null ? cam.camera_id : cam.id;
          var card = document.createElement('article');
          card.className = 'camera-card';
          card.innerHTML =
            (isOnline
              ? '<div class="camera-scene"><span class="scene-building"></span><span class="scene-ground"></span><span class="camera-live">● LIVE</span><span class="ai-id-tag">ID #' + camId + '</span></div>'
              : '<div class="camera-scene scene-offline"><span class="camera-live offline-tag"><span class="dot offline"></span>OFFLINE</span><span class="ai-id-tag">ID #' + camId + '</span><span class="offline-copy">Tạm thời chưa hoạt động</span></div>') +
            '<div class="camera-info"><div><strong></strong><small></small></div><span class="status ' + (isOnline ? 'online' : 'unknown') + '"><span class="dot ' + (isOnline ? '' : 'offline') + '"></span>' + (isOnline ? 'Đang hoạt động' : 'Tạm thời chưa hoạt động') + '</span></div>' +
            '<button class="btn camera-detail">Xem chi tiết</button>';
          card.querySelector('strong').textContent = cam.name;
          card.querySelector('small').textContent = 'camera_id: #' + camId + ' · ' + (cam.location || 'Chưa đặt vị trí') + (cam.ip_address ? ' · ' + cam.ip_address : '');
          card.querySelector('.camera-detail').addEventListener('click', function () {
            openCameraDetailModal(cam);
          });
          gallery.appendChild(card);
        });
      }
    }

    // Cameras management table
    var mgmtSearch = document.getElementById('ss-cam-mgmt-search');
    var mgmtStatus = document.getElementById('ss-cam-mgmt-status');
    var mq = mgmtSearch ? mgmtSearch.value.trim().toLowerCase() : '';
    var mloc = mgmtLocSelect ? mgmtLocSelect.value : 'all';
    var mst = mgmtStatus ? mgmtStatus.value : 'all';
    var hasMgmtFilter = Boolean(mq || mloc !== 'all' || mst !== 'all');

    var filteredMgmt = (hasMgmtFilter && Array.isArray(state.mgmtCameras))
      ? state.mgmtCameras
      : items.filter(function (cam) {
          var matchQ = !mq || (cam.name + ' ' + (cam.location || '') + ' ' + (cam.ip_address || '')).toLowerCase().indexOf(mq) >= 0;
          var matchLoc = mloc === 'all' || (cam.location || '').toLowerCase() === mloc.toLowerCase();
          var matchSt = mst === 'all' || cam.status === mst;
          return matchQ && matchLoc && matchSt;
        });

    var camBody = document.querySelector('[data-page="cameras"] .table tbody');
    var camFoot = document.querySelector('[data-page="cameras"] .table-foot span');
    if (camFoot) camFoot.textContent = 'Tổng cộng ' + filteredMgmt.length + ' camera';
    if (camBody) {
      camBody.replaceChildren();
      if (!filteredMgmt.length) {
        camBody.appendChild(emptyRow(7, items.length ? 'Không có camera khớp bộ lọc.' : 'Chưa có camera nào trong cơ sở dữ liệu.'));
      } else {
        filteredMgmt.forEach(function (cam) {
          var tr = document.createElement('tr');
          var isOnline = cam.status === 'ONLINE';
          var camId = cam.camera_id != null ? cam.camera_id : cam.id;
          tr.innerHTML =
            '<td><span class="cam-id-badge">#' + camId + '</span></td>' +
            '<td><strong></strong><small style="display:block;color:var(--ss-muted);font-family:var(--ss-font-mono);font-size:11px;margin-top:2px"></small></td>' +
            '<td></td><td style="font-family:var(--ss-font-mono);font-size:12px"></td>' +
            '<td><span class="status ' + (isOnline ? 'online' : 'unknown') + '"><span class="dot ' + (isOnline ? '' : 'offline') + '"></span>' + (isOnline ? 'ONLINE' : 'OFFLINE') + '</span></td>' +
            '<td style="font-family:var(--ss-font-mono);font-size:11.5px"></td><td><div class="table-actions"></div></td>';
          var tds = tr.querySelectorAll('td');
          var actionsWrap = tds[6].querySelector('.table-actions');
          tds[1].querySelector('strong').textContent = cam.name;
          tds[1].querySelector('small').textContent = 'camera_id: ' + camId + ' · ' + (cam.ip_address || 'IP chưa đặt');
          tds[2].textContent = cam.location || '—';
          tds[3].textContent = cam.rtsp_url || '—';
          if (cam.rtsp_has_credentials) {
            var shield = document.createElement('span');
            shield.className = 'rtsp-shield-tag';
            shield.textContent = 'Đã che mật khẩu';
            tds[3].appendChild(shield);
          }
          tds[5].textContent = cam.last_seen ? new Date(cam.last_seen).toLocaleString('vi-VN') : 'Chưa kết nối';

          if (isAdmin()) {
            var editBtn = document.createElement('button');
            editBtn.className = 'btn subtle';
            editBtn.setAttribute('data-edit-cam', camId);
            editBtn.textContent = 'Sửa';
            editBtn.addEventListener('click', function () { openCameraFormModal(cam); });

            var delBtn = document.createElement('button');
            delBtn.className = 'btn danger';
            delBtn.setAttribute('data-del-cam', camId);
            delBtn.textContent = 'Xóa';
            delBtn.addEventListener('click', function () {
              confirmAction('Bạn có chắc muốn xóa camera "' + cam.name + '" (camera_id #' + camId + ')?', function () {
                request('/cameras/' + camId, { method: 'DELETE' }).then(function () {
                  state.monitorCameras = null;
                  state.mgmtCameras = null;
                  toast('Đã xóa camera thành công.');
                  refreshDataOnly();
                }).catch(function (err) {
                  toast(err.detail || 'Không thể xóa camera.');
                });
              });
            });
            actionsWrap.appendChild(editBtn);
            actionsWrap.appendChild(delBtn);
          } else {
            actionsWrap.innerHTML = '<span class="status unknown">Chỉ xem</span>';
          }
          camBody.appendChild(tr);
        });
      }
    }
  }

  /* ==================== 3. AI MONITORING (SNAPSHOTS, TABLE, DETAIL MODAL, REALTIME) ==================== */
  function attachSnapshotElement(container, imagePath, altText, isThumb) {
    var url = resolveUploadUrl(imagePath);
    function showFallback() {
      container.classList.remove('has-snapshot');
      var fb = document.createElement('div');
      fb.className = isThumb ? 'ai-thumb-fallback' : 'ai-snapshot-fallback';
      fb.innerHTML = isThumb
        ? 'No img'
        : '<span style="font-size:20px;opacity:0.8">📷</span><span>Chưa có ảnh snapshot</span>';
      container.appendChild(fb);
    }
    if (!url) {
      showFallback();
      return;
    }
    container.classList.add('has-snapshot');
    var img = document.createElement('img');
    img.className = isThumb ? '' : 'ai-snapshot';
    img.src = url;
    img.alt = altText || 'Snapshot AI';
    img.loading = 'lazy';
    img.addEventListener('error', function () {
      img.remove();
      showFallback();
    });
    container.appendChild(img);
  }

  function openDetectionDetailModal(eventId) {
    var backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML =
      '<div class="modal detection-detail-modal" role="dialog" aria-modal="true">' +
      '<div class="modal-head"><span>Chi tiết sự kiện AI #' + eventId + '</span><button class="modal-close" aria-label="Đóng">×</button></div>' +
      '<div class="detection-detail-body" style="color:var(--ss-muted);padding:16px 0;text-align:center">Đang tải chi tiết sự kiện từ máy chủ...</div>' +
      '</div>';
    document.getElementById('ss-modal-root').appendChild(backdrop);
    function close() { backdrop.remove(); }
    backdrop.querySelector('.modal-close').addEventListener('click', close);
    backdrop.addEventListener('click', function (e) { if (e.target === backdrop) close(); });

    request('/detections/' + eventId).then(function (det) {
      var body = backdrop.querySelector('.detection-detail-body');
      body.style.textAlign = 'left';
      body.style.padding = '0';
      body.style.color = 'inherit';

      var pct = ((det.confidence || 0) * 100).toFixed(1);
      var evLower = String(det.event_type || '').toLowerCase();
      var isFire = evLower === 'fire' || evLower === 'flame';
      var isSmoke = evLower === 'smoke';
      var badgeCls = (isFire || (isSmoke && (det.confidence || 0) >= 0.85)) ? 'critical' : (isSmoke ? 'warning' : 'info');

      body.innerHTML =
        '<div class="detection-detail-media"></div>' +
        '<div class="settings-list">' +
        '<div><span>ID sự kiện</span><span class="cam-id-badge">#' + det.id + '</span></div>' +
        '<div><span>Camera</span><strong data-det-cam></strong></div>' +
        '<div><span>Khu vực lắp đặt</span><strong data-det-loc></strong></div>' +
        '<div><span>Đối tượng nhận diện</span><strong data-det-obj></strong></div>' +
        '<div><span>Loại sự kiện</span><span class="status ' + badgeCls + '">' + String(det.event_type || 'object').toUpperCase() + '</span></div>' +
        '<div><span>Độ tin cậy AI</span><strong style="font-family:var(--ss-font-mono)">' + pct + '%</strong></div>' +
        '<div><span>Thời điểm phát hiện</span><strong data-det-time style="font-family:var(--ss-font-mono);font-size:12px"></strong></div>' +
        '<div><span>Đường dẫn ảnh snapshot</span><strong data-det-path style="font-family:var(--ss-font-mono);font-size:11.5px;word-break:break-all"></strong></div>' +
        '</div>' +
        '<div class="detection-alerts-box" data-det-alerts></div>';

      attachSnapshotElement(
        body.querySelector('.detection-detail-media'),
        det.image_url || det.image_path,
        'Snapshot sự kiện #' + det.id,
        false
      );
      body.querySelector('[data-det-cam]').textContent = (det.camera_name || ('Camera #' + det.camera_id)) + ' (camera_id: #' + det.camera_id + ')';
      body.querySelector('[data-det-loc]').textContent = det.camera_location || 'Chưa đặt vị trí';
      body.querySelector('[data-det-obj]').textContent = det.object_type || '—';
      body.querySelector('[data-det-time]').textContent = det.timestamp ? new Date(det.timestamp).toLocaleString('vi-VN') : '—';
      body.querySelector('[data-det-path]').textContent = det.image_path || 'Chưa lưu ảnh snapshot';

      var alertsBox = body.querySelector('[data-det-alerts]');
      if (det.alerts && det.alerts.length) {
        alertsBox.innerHTML = '<div style="font-size:12px;font-weight:700;margin-bottom:8px">Cảnh báo an ninh liên quan (' + det.alerts.length + ')</div>';
        det.alerts.forEach(function (al) {
          var row = document.createElement('div');
          row.style.display = 'flex';
          row.style.alignItems = 'center';
          row.style.justifyContent = 'space-between';
          row.style.gap = '8px';
          row.style.padding = '6px 0';
          row.style.fontSize = '12px';
          row.innerHTML = '<span></span><span>' + formatSeverityBadge(al.severity) + ' ' + formatAlertStatusBadge(al.status) + '</span>';
          row.querySelector('span').textContent = '#' + al.id + ' · ' + al.description;
          alertsBox.appendChild(row);
        });
      } else {
        alertsBox.innerHTML = '<div style="font-size:12px;color:var(--ss-muted)">Sự kiện này không kích hoạt cảnh báo mới (dưới ngưỡng hoặc được gộp trùng).</div>';
      }
    }).catch(function (err) {
      var body = backdrop.querySelector('.detection-detail-body');
      body.textContent = err.detail || 'Không thể tải chi tiết sự kiện AI.';
    });
  }

  function fetchFilteredDetections() {
    if (!token) return;
    var camSel = document.getElementById('ss-ai-camera-filter');
    var evSel = document.getElementById('ss-ai-event-filter');
    var confSel = document.getElementById('ss-ai-conf-filter');

    var qs = ['limit=100'];
    if (camSel && camSel.value !== 'all') qs.push('camera_id=' + encodeURIComponent(camSel.value));
    if (state.aiChipFilter && state.aiChipFilter !== 'all') qs.push('object_type=' + encodeURIComponent(state.aiChipFilter));
    if (evSel && evSel.value !== 'all') qs.push('event_type=' + encodeURIComponent(evSel.value));
    if (confSel && parseFloat(confSel.value) > 0) qs.push('min_confidence=' + encodeURIComponent(confSel.value));

    request('/detections?' + qs.join('&')).then(function (res) {
      state.filteredDetections = (res && res.items) || [];
      renderAiView();
    }).catch(function () {});
  }

  function renderAiView() {
    var detItems = state.detections;
    var aiHeadPill = document.getElementById('ss-ai-head-pill');
    var aiActive = (state.health && (state.health.ai_worker === 'online' || state.health.ai === 'online')) || detItems.length > 0;
    if (aiHeadPill) {
      aiHeadPill.className = 'status ' + (aiActive ? 'online' : 'unknown');
      aiHeadPill.innerHTML = aiActive
        ? '<i class="dot"></i>AI Worker: Đang hoạt động'
        : '<i class="dot offline"></i>AI Worker: Tạm thời chưa hoạt động';
    }

    // Đồng bộ dropdown chọn Camera trong trang AI
    var camFilter = document.getElementById('ss-ai-camera-filter');
    if (camFilter) {
      var prevCam = camFilter.value || 'all';
      camFilter.replaceChildren();
      var optAll = document.createElement('option');
      optAll.value = 'all';
      optAll.textContent = 'Tất cả Camera';
      camFilter.appendChild(optAll);
      state.cameras.forEach(function (c) {
        var cid = c.camera_id != null ? c.camera_id : c.id;
        var opt = document.createElement('option');
        opt.value = String(cid);
        opt.textContent = '#' + cid + ' · ' + c.name;
        camFilter.appendChild(opt);
      });
      if (prevCam !== 'all' && state.cameras.some(function (c) { return String(c.id) === prevCam; })) {
        camFilter.value = prevCam;
      }
    }

    var fireCount = detItems.filter(function (d) {
      var t = String(d.object_type || '').toLowerCase();
      return t === 'fire' || t === 'flame';
    }).length;
    var smokeCount = detItems.filter(function (d) {
      return String(d.object_type || '').toLowerCase() === 'smoke';
    }).length;
    var otherCount = Math.max(0, detItems.length - fireCount - smokeCount);

    var aiCountAll = document.querySelector('[data-ai-count="all"]');
    var aiCountFire = document.querySelector('[data-ai-count="fire"]');
    var aiCountSmoke = document.querySelector('[data-ai-count="smoke"]');
    var aiCountOther = document.querySelector('[data-ai-count="other"]');
    if (aiCountAll) aiCountAll.textContent = detItems.length;
    if (aiCountFire) aiCountFire.textContent = fireCount;
    if (aiCountSmoke) aiCountSmoke.textContent = smokeCount;
    if (aiCountOther) aiCountOther.textContent = otherCount;

    var evFilter = document.getElementById('ss-ai-event-filter');
    var confFilter = document.getElementById('ss-ai-conf-filter');
    var searchIn = document.getElementById('ss-ai-search');
    var camVal = camFilter ? camFilter.value : 'all';
    var evVal = evFilter ? evFilter.value : 'all';
    var minConf = confFilter ? (parseFloat(confFilter.value) || 0) : 0;
    var q = searchIn ? searchIn.value.trim().toLowerCase() : '';
    var hasServerFilter = Boolean(camVal !== 'all' || state.aiChipFilter !== 'all' || evVal !== 'all' || minConf > 0);

    var sourceList = (hasServerFilter && Array.isArray(state.filteredDetections))
      ? state.filteredDetections
      : detItems;

    var filtered = sourceList.filter(function (d) {
      var objLower = String(d.object_type || '').toLowerCase();
      var evLower = String(d.event_type || '').toLowerCase();
      var isFire = objLower === 'fire' || objLower === 'flame';
      var isSmoke = objLower === 'smoke';
      var matchCam = camVal === 'all' || String(d.camera_id) === camVal;
      var matchChip =
        state.aiChipFilter === 'all' ||
        (state.aiChipFilter === 'fire' && isFire) ||
        (state.aiChipFilter === 'smoke' && isSmoke) ||
        (state.aiChipFilter === 'other' && !isFire && !isSmoke);
      var matchEv =
        evVal === 'all' ||
        (evVal === 'fire' && (evLower === 'fire' || evLower === 'flame')) ||
        (evVal === 'smoke' && evLower === 'smoke') ||
        (evVal === 'other' && evLower !== 'fire' && evLower !== 'flame' && evLower !== 'smoke');
      var matchConf = (d.confidence || 0) >= minConf;
      var matchQ = !q || (d.object_type + ' ' + d.event_type + ' ' + (d.camera_name || '') + ' ' + d.camera_id).toLowerCase().indexOf(q) >= 0;
      return matchCam && matchChip && matchEv && matchConf && matchQ;
    });

    var aiGrid = document.querySelector('[data-page="ai"] .ai-grid');
    var aiTableBody = document.getElementById('ss-ai-table-body');
    var aiFoot = document.querySelector('[data-page="ai"] .table-foot span');
    if (aiFoot) aiFoot.textContent = 'Tổng cộng ' + filtered.length + ' sự kiện';

    if (aiGrid) {
      aiGrid.replaceChildren();
      if (!filtered.length) {
        var empty = document.createElement('div');
        empty.style.color = '#64748b';
        empty.style.padding = '24px';
        empty.innerHTML = '<span class="dot offline"></span>' + (detItems.length ? 'Không có sự kiện Lửa/Khói khớp bộ lọc.' : 'Tạm thời chưa hoạt động — Chưa kết nối luồng Camera RTSP với AI Worker (YOLOv8 Lửa & Khói).');
        aiGrid.appendChild(empty);
      } else {
        filtered.slice(0, 12).forEach(function (d) {
          var pct = ((d.confidence || 0) * 100).toFixed(1);
          var evLower = String(d.event_type || '').toLowerCase();
          var isFire = evLower === 'fire' || evLower === 'flame';
          var isSmoke = evLower === 'smoke';
          var badgeCls = (isFire || (isSmoke && (d.confidence || 0) >= 0.85)) ? 'critical' : (isSmoke ? 'warning' : 'info');
          var card = document.createElement('article');
          card.className = 'ai-card';
          card.title = 'Nhấn để xem chi tiết sự kiện #' + d.id;
          card.innerHTML =
            '<div class="ai-image"><span class="ai-img-badge"></span><span class="ai-id-tag">#' + d.id + '</span></div>' +
            '<div class="ai-meta"><div class="ai-title"><strong></strong><span class="status ' + badgeCls + '">' + String(d.event_type || 'other').toUpperCase() + '</span></div>' +
            '<div class="ai-sub"></div>' +
            '<div class="confidence"><i style="width:' + pct + '%;background:' + (isFire ? '#ef4444' : (isSmoke ? '#f59e0b' : '#3b82f6')) + '"></i></div>' +
            '<div class="ai-foot"><span>Độ tin cậy · Nhấn xem chi tiết</span><strong>' + pct + '%</strong></div></div>';
          var imgWrap = card.querySelector('.ai-image');
          attachSnapshotElement(imgWrap, d.image_path, d.object_type + ' - ' + (d.camera_name || ('CAM #' + d.camera_id)), false);
          card.querySelector('.ai-img-badge').textContent = (isFire ? '🔥 LỬA' : (isSmoke ? '💨 KHÓI' : d.object_type)) + ' · ' + pct + '%';
          card.querySelector('.ai-title strong').textContent = (isFire ? 'Lửa (fire)' : (isSmoke ? 'Khói (smoke)' : d.object_type));
          card.querySelector('.ai-sub').textContent = (d.camera_name || ('CAM #' + d.camera_id)) + ' (ID #' + d.camera_id + ') · ' + new Date(d.timestamp).toLocaleString('vi-VN');
          card.addEventListener('click', function () { openDetectionDetailModal(d.id); });
          aiGrid.appendChild(card);
        });
      }
    }

    if (aiTableBody) {
      aiTableBody.replaceChildren();
      if (!filtered.length) {
        aiTableBody.appendChild(emptyRow(8, detItems.length ? 'Không có sự kiện Lửa/Khói khớp bộ lọc.' : 'Chưa có sự kiện AI nào được ghi nhận.'));
      } else {
        filtered.forEach(function (d) {
          var pct = ((d.confidence || 0) * 100).toFixed(1);
          var evLower = String(d.event_type || '').toLowerCase();
          var isFire = evLower === 'fire' || evLower === 'flame';
          var isSmoke = evLower === 'smoke';
          var badgeCls = (isFire || (isSmoke && (d.confidence || 0) >= 0.85)) ? 'critical' : (isSmoke ? 'warning' : 'info');
          var tr = document.createElement('tr');
          tr.innerHTML =
            '<td><span class="cam-id-badge">#' + d.id + '</span></td>' +
            '<td><div class="ai-thumb-box"></div></td>' +
            '<td><strong></strong><small style="display:block;color:var(--ss-muted);font-family:var(--ss-font-mono);font-size:11px"></small></td>' +
            '<td><strong></strong></td>' +
            '<td><span class="status ' + badgeCls + '">' + String(d.event_type || 'other').toUpperCase() + '</span></td>' +
            '<td style="font-family:var(--ss-font-mono);font-weight:700">' + pct + '%</td>' +
            '<td style="font-family:var(--ss-font-mono);font-size:11.5px"></td>' +
            '<td><div class="table-actions"><button class="btn subtle">Chi tiết</button></div></td>';
          var tds = tr.querySelectorAll('td');
          attachSnapshotElement(tds[1].querySelector('.ai-thumb-box'), d.image_path, 'Snapshot #' + d.id, true);
          tds[2].querySelector('strong').textContent = d.camera_name || ('Camera #' + d.camera_id);
          tds[2].querySelector('small').textContent = 'camera_id: #' + d.camera_id;
          tds[3].querySelector('strong').textContent = isFire ? 'Lửa (fire)' : (isSmoke ? 'Khói (smoke)' : d.object_type);
          tds[6].textContent = d.timestamp ? new Date(d.timestamp).toLocaleString('vi-VN') : '—';
          tds[7].querySelector('button').addEventListener('click', function () {
            openDetectionDetailModal(d.id);
          });
          aiTableBody.appendChild(tr);
        });
      }
    }
  }

  /* ==================== 4. IOT DEVICES ==================== */
  function getActiveDevice() {
    if (!state.devices.length) return null;
    var found = state.devices.find(function (d) { return d.device_code === state.selectedDeviceCode; });
    if (found) return found;
    state.selectedDeviceCode = state.devices[0].device_code;
    return state.devices[0];
  }

  function renderIotView() {
    var devItems = state.devices;
    var iotPill = document.querySelector('[data-page="iot"] .head-actions .system-pill');
    var devOnline = devItems.filter(function (d) { return d.status === 'ONLINE'; }).length;
    if (iotPill) iotPill.innerHTML = '<i class="dot"></i>' + devOnline + ' / ' + devItems.length + ' trực tuyến';

    var devSelect = document.getElementById('ss-iot-device-select');
    if (devSelect) {
      if (devItems.length > 0) {
        devSelect.style.display = '';
        devSelect.innerHTML = devItems.map(function (d) {
          return '<option value="' + d.device_code + '">' + d.device_code + ' · ' + d.device_name + '</option>';
        }).join('');
        var active = getActiveDevice();
        if (active) devSelect.value = active.device_code;
      } else {
        devSelect.style.display = 'none';
      }
    }

    var iotHeadPill = document.getElementById('ss-iot-head-pill');
    var devOnlineCount = devItems.filter(function (d) { return d.status === 'ONLINE'; }).length;
    if (iotHeadPill) {
      if (devOnlineCount > 0) {
        iotHeadPill.className = 'status online';
        iotHeadPill.innerHTML = '<i class="dot"></i>Đang hoạt động (' + devOnlineCount + '/' + devItems.length + ' trực tuyến)';
      } else {
        iotHeadPill.className = 'status unknown';
        iotHeadPill.innerHTML = '<i class="dot offline"></i>Tạm thời chưa hoạt động (0/' + devItems.length + ' trực tuyến)';
      }
    }

    var activeDev = getActiveDevice();
    var note = document.getElementById('ss-iot-sensor-note');
    var badge = document.getElementById('ss-iot-sensor-badge');
    var pirVal = document.getElementById('ss-pir-val');
    var pirDesc = document.getElementById('ss-pir-desc');
    var pirStatus = document.getElementById('ss-pir-status');
    var mq2Val = document.getElementById('ss-mq2-val');
    var mq2Bar = document.getElementById('ss-mq2-bar');
    var mq2Status = document.getElementById('ss-mq2-status');
    var lockVal = document.getElementById('ss-lock-val');
    var relayDesc = document.getElementById('ss-relay-desc');
    var actStatus = document.getElementById('ss-actuator-status');
    var cmdTarget = document.getElementById('ss-iot-cmd-target');

    var toggleRelay = document.getElementById('ss-toggle-relay');
    var toggleBuzzer = document.getElementById('ss-toggle-buzzer');
    var btnLock = document.getElementById('ss-btn-lock-toggle');
    var relayText = document.getElementById('ss-relay-state-text');
    var buzzerText = document.getElementById('ss-buzzer-state-text');
    var lockText = document.getElementById('ss-lock-state-text');

    var isDevActive = Boolean(activeDev && (activeDev.status === 'ONLINE' || activeDev.latest_pir !== null || activeDev.latest_mq2 !== null));

    if (!isDevActive) {
      if (note) note.textContent = activeDev ? (activeDev.device_name + ' (' + activeDev.device_code + ') · Đang ngoại tuyến') : 'Tạm thời chưa hoạt động (Chưa có thiết bị kết nối)';
      if (badge) { badge.className = 'status unknown'; badge.innerHTML = '<span class="dot offline"></span>Tạm thời chưa hoạt động'; }
      if (pirVal) { pirVal.textContent = '(—)'; pirVal.style.color = '#64748b'; }
      if (pirDesc) pirDesc.textContent = 'Chưa có tín hiệu từ cảm biến Lửa (Flame IR)';
      if (pirStatus) { pirStatus.className = 'status unknown'; pirStatus.innerHTML = '<span class="dot offline"></span>Tạm thời chưa hoạt động'; }
      if (mq2Val) { mq2Val.innerHTML = '(— <small>ppm</small>)'; mq2Val.style.color = '#64748b'; }
      if (mq2Bar && mq2Bar.parentElement) { mq2Bar.parentElement.style.display = 'none'; }
      if (mq2Status) { mq2Status.className = 'status unknown'; mq2Status.innerHTML = '<span class="dot offline"></span>Tạm thời chưa hoạt động'; }
      if (lockVal) { lockVal.textContent = '(—)'; lockVal.style.color = '#64748b'; }
      if (relayDesc) relayDesc.textContent = 'Chưa kết nối Mạch Relay / Còi 12V / Khóa từ Solenoid';
      if (actStatus) { actStatus.className = 'status unknown'; actStatus.innerHTML = '<span class="dot offline"></span>Tạm thời chưa hoạt động'; }
      if (cmdTarget) { cmdTarget.className = 'status unknown'; cmdTarget.innerHTML = '<span class="dot offline"></span>Tạm thời chưa hoạt động'; }
      if (toggleRelay) { toggleRelay.classList.remove('on'); toggleRelay.disabled = true; toggleRelay.style.opacity = '0.45'; }
      if (toggleBuzzer) { toggleBuzzer.classList.remove('on'); toggleBuzzer.disabled = true; toggleBuzzer.style.opacity = '0.45'; }
      if (btnLock) { btnLock.disabled = true; btnLock.style.opacity = '0.45'; }
      if (relayText) relayText.textContent = 'Tạm thời chưa hoạt động (Chờ mạch ESP32 kết nối)';
      if (buzzerText) buzzerText.textContent = 'Tạm thời chưa hoạt động (Chờ mạch ESP32 kết nối)';
      if (lockText) lockText.textContent = 'Tạm thời chưa hoạt động (Chờ mạch ESP32 kết nối)';
    } else {
      if (note) note.textContent = activeDev.device_name + ' (' + activeDev.device_code + ') · Đang kết nối trực tuyến';
      if (badge) { badge.className = 'status online'; badge.innerHTML = '<span class="dot"></span>Đang hoạt động'; }
      if (cmdTarget) { cmdTarget.className = 'status online'; cmdTarget.innerHTML = '<span class="dot"></span>' + activeDev.device_code + ' · Đang hoạt động'; }

      var flameDetected = Boolean(activeDev.latest_pir);
      if (pirVal) {
        pirVal.textContent = flameDetected ? 'PHÁT HIỆN TIA LỬA!' : 'An toàn';
        pirVal.style.color = flameDetected ? '#dc2626' : '#10b981';
      }
      if (pirDesc) pirDesc.textContent = flameDetected ? 'Cảm biến Flame IR phát hiện bức xạ lửa tại chỗ!' : 'Không phát hiện tia lửa bất thường (GPIO 27)';
      if (pirStatus) {
        pirStatus.className = 'status ' + (flameDetected ? 'critical' : 'online');
        pirStatus.innerHTML = flameDetected ? '<span class="dot red"></span>BÁO CHÁY' : '<span class="dot"></span>Đang hoạt động';
      }

      var mq2 = activeDev.latest_mq2 != null ? Number(activeDev.latest_mq2) : 0;
      var mq2Thresh = Number(state.settings.mq2_threshold || 600);
      var mq2Danger = mq2 >= mq2Thresh;
      var mq2Pct = Math.min(100, Math.round((mq2 / (mq2Thresh * 1.5)) * 100));
      if (mq2Val) {
        mq2Val.innerHTML = mq2 + ' <small>ppm</small>';
        mq2Val.style.color = mq2Danger ? '#dc2626' : '#0f172a';
      }
      if (mq2Bar && mq2Bar.parentElement) {
        mq2Bar.parentElement.style.display = 'block';
        mq2Bar.style.width = mq2Pct + '%';
        mq2Bar.style.background = mq2Danger ? '#dc2626' : '#10b981';
      }
      if (mq2Status) {
        mq2Status.className = 'status ' + (mq2Danger ? 'critical' : 'online');
        mq2Status.innerHTML = mq2Danger ? '<span class="dot red"></span>Vượt ngưỡng!' : '<span class="dot"></span>Đang hoạt động';
      }

      var locked = activeDev.lock_engaged !== false;
      var relayOn = Boolean(activeDev.relay);
      var buzzerOn = Boolean(activeDev.buzzer);
      if (lockVal) {
        lockVal.textContent = locked ? 'Đang khóa' : 'Đã mở thoát hiểm';
        lockVal.style.color = locked ? '#10b981' : '#dc2626';
      }
      if (relayDesc) relayDesc.textContent = 'Relay Bơm: ' + (relayOn ? 'Bật' : 'Tắt') + ' · Còi 12V: ' + (buzzerOn ? 'Bật' : 'Tắt');
      if (actStatus) { actStatus.className = 'status online'; actStatus.innerHTML = '<span class="dot"></span>Đang hoạt động'; }

      if (toggleRelay) { toggleRelay.classList.toggle('on', relayOn); toggleRelay.disabled = false; toggleRelay.style.opacity = '1'; }
      if (toggleBuzzer) { toggleBuzzer.classList.toggle('on', buzzerOn); toggleBuzzer.disabled = false; toggleBuzzer.style.opacity = '1'; }
      if (btnLock) { btnLock.disabled = false; btnLock.style.opacity = '1'; btnLock.textContent = locked ? 'Mở thoát hiểm' : 'Khóa Solenoid'; }
      if (relayText) relayText.textContent = 'Bơm chữa cháy / Phun sương: ' + (relayOn ? 'Đang KÍCH HOẠT' : 'Đang TẮT');
      if (buzzerText) buzzerText.textContent = 'Còi báo động 12V: ' + (buzzerOn ? 'Đang HÚ CÒI' : 'Đang TẮT');
      if (lockText) lockText.textContent = 'Khóa từ Solenoid thoát hiểm: ' + (locked ? 'Đang KHÓA' : 'ĐÃ NHẢ MỞ CỬA THOÁT HIỂM');
    }

    var iotBody = document.querySelector('[data-page="iot"] .table tbody');
    if (iotBody) {
      iotBody.replaceChildren();
      if (!devItems.length) {
        iotBody.appendChild(emptyRow(7, 'Chưa có thiết bị ESP32 PCCC nào được đăng ký.'));
      } else {
        devItems.forEach(function (d) {
          var tr = document.createElement('tr');
          var isOnline = d.status === 'ONLINE';
          tr.innerHTML =
            '<td><strong style="font-family:var(--ss-font-mono)"></strong></td><td></td><td></td>' +
            '<td><span class="status ' + (isOnline ? 'online' : 'unknown') + '"><span class="dot ' + (isOnline ? '' : 'offline') + '"></span>' + d.status + '</span></td>' +
            '<td style="font-family:var(--ss-font-mono);font-size:12px"></td><td style="font-family:var(--ss-font-mono);font-size:11.5px"></td><td><div class="table-actions"></div></td>';
          var tds = tr.querySelectorAll('td');
          var actionsWrap = tds[6].querySelector('.table-actions');
          tds[0].querySelector('strong').textContent = d.device_code;
          tds[1].textContent = d.device_name;
          tds[2].textContent = d.device_type;
          tds[4].textContent = 'Flame: ' + (d.latest_pir ? 'Có lửa!' : 'An toàn') + ' · MQ-2: ' + (d.latest_mq2 != null ? d.latest_mq2 + ' ppm' : '—');
          tds[5].textContent = d.last_seen ? new Date(d.last_seen).toLocaleString('vi-VN') : 'Chưa kết nối';

          var selBtn = document.createElement('button');
          selBtn.className = (activeDev && activeDev.device_code === d.device_code) ? 'btn primary' : 'btn subtle';
          selBtn.textContent = (activeDev && activeDev.device_code === d.device_code) ? 'Đang chọn' : 'Chọn';
          selBtn.addEventListener('click', function () {
            state.selectedDeviceCode = d.device_code;
            renderIotView();
          });
          actionsWrap.appendChild(selBtn);

          if (isAdmin()) {
            var delBtn = document.createElement('button');
            delBtn.className = 'btn danger';
            delBtn.setAttribute('data-del-dev', d.device_code);
            delBtn.textContent = 'Xóa';
            delBtn.addEventListener('click', function () {
              confirmAction('Xóa thiết bị "' + d.device_code + '" khỏi hệ thống?', function () {
                request('/devices/' + encodeURIComponent(d.device_code), { method: 'DELETE' }).then(function () {
                  toast('Đã xóa thiết bị ' + d.device_code);
                  refreshDataOnly();
                }).catch(function (err) {
                  toast(err.detail || 'Không thể xóa thiết bị.');
                });
              });
            });
            actionsWrap.appendChild(delBtn);
          }
          iotBody.appendChild(tr);
        });
      }
    }
  }

  /* ==================== 5. ALERTS ==================== */
  function renderAlertsView() {
    var alertItems = state.alerts;
    var totalAlerts = alertItems.length;
    var openAlerts = alertItems.filter(function (a) { return a.status === 'OPEN'; }).length;
    var critAlerts = alertItems.filter(function (a) { return a.status === 'OPEN' && a.severity === 'CRITICAL'; }).length;
    var resolvedAlerts = alertItems.filter(function (a) { return a.status === 'RESOLVED'; }).length;
    var resolvedPct = totalAlerts > 0 ? Math.round((resolvedAlerts / totalAlerts) * 100) : 0;

    var elTotal = document.querySelector('[data-alert-stat="total"]');
    var elOpen = document.querySelector('[data-alert-stat="open"]');
    var elCrit = document.querySelector('[data-alert-stat="critical"]');
    var elRes = document.querySelector('[data-alert-stat="resolved"]');
    var elResPct = document.querySelector('[data-alert-stat="resolved-pct"]');
    if (elTotal) elTotal.textContent = totalAlerts;
    if (elOpen) elOpen.textContent = openAlerts;
    if (elCrit) elCrit.textContent = critAlerts;
    if (elRes) elRes.textContent = resolvedAlerts;
    if (elResPct) elResPct.textContent = resolvedPct + '% tổng số cảnh báo';

    var searchIn = document.getElementById('ss-alert-search');
    var sevIn = document.getElementById('ss-alert-severity');
    var stIn = document.getElementById('ss-alert-status');
    var q = searchIn ? searchIn.value.trim().toLowerCase() : '';
    var sev = sevIn ? sevIn.value : 'all';
    var st = stIn ? stIn.value : 'all';

    var filtered = alertItems.filter(function (a) {
      var codeStr = '#alt-' + String(a.id).padStart(4, '0');
      var matchQ = !q || (codeStr + ' ' + a.description + ' ' + a.alert_type).toLowerCase().indexOf(q) >= 0;
      var matchSev = sev === 'all' || a.severity === sev;
      var matchSt = st === 'all' || a.status === st;
      return matchQ && matchSev && matchSt;
    });

    var alertBody = document.querySelector('[data-page="alerts"] .table tbody');
    var alertFoot = document.querySelector('[data-page="alerts"] .table-foot span');
    if (alertFoot) alertFoot.textContent = 'Tổng cộng ' + filtered.length + ' cảnh báo';
    if (alertBody) {
      alertBody.replaceChildren();
      if (!filtered.length) {
        alertBody.appendChild(emptyRow(9, alertItems.length ? 'Không có cảnh báo khớp bộ lọc.' : 'Chưa có cảnh báo nào.'));
      } else {
        filtered.forEach(function (a) {
          var tr = document.createElement('tr');
          tr.innerHTML = '<td><input type="checkbox" class="ss-alert-row-check"></td><td><strong>#ALT-' + String(a.id).padStart(4, '0') + '</strong></td><td></td><td></td><td></td><td></td><td></td><td></td><td></td>';
          var tds = tr.querySelectorAll('td');
          tds[2].textContent = a.description;
          tds[3].textContent = a.alert_type;
          tds[4].appendChild(formatSeverityBadge(a.severity));
          tds[5].textContent = new Date(a.created_at).toLocaleString('vi-VN');
          tds[6].textContent = a.handled_by_name || (a.status === 'RESOLVED' ? 'Đã xử lý' : '—');
          tds[7].appendChild(formatAlertStatusBadge(a.status));
          if (a.status === 'OPEN') {
            var btn = document.createElement('button');
            btn.className = 'btn subtle';
            btn.textContent = 'Xử lý';
            btn.addEventListener('click', function () {
              request('/alerts/' + a.id + '/resolve', { method: 'PUT' }).then(function () {
                toast('Đã đánh dấu xử lý cảnh báo #ALT-' + String(a.id).padStart(4, '0'));
                refreshDataOnly();
              }).catch(function (err) {
                toast(err.detail || 'Không thể xử lý cảnh báo.');
              });
            });
            tds[8].appendChild(btn);
          } else {
            tds[8].textContent = 'Hoàn tất';
          }
          alertBody.appendChild(tr);
        });
      }
    }
  }

  /* ==================== 6. HISTORY ==================== */
  function renderHistoryView() {
    var detItems = state.detections;
    var devItems = state.devices;
    var logItems = state.logs;
    var alertItems = state.alerts;

    var histAi = document.querySelector('[data-hist-count="ai"]');
    var histIot = document.querySelector('[data-hist-count="iot"]');
    var histLogs = document.querySelector('[data-hist-count="logs"]');
    var histAlerts = document.querySelector('[data-hist-count="alerts"]');
    if (histAi) histAi.textContent = detItems.length;
    if (histIot) histIot.textContent = devItems.length;
    if (histLogs) histLogs.textContent = logItems.length;
    if (histAlerts) histAlerts.textContent = alertItems.length;

    var searchIn = document.getElementById('ss-hist-search');
    var typeIn = document.getElementById('ss-hist-type');
    var q = searchIn ? searchIn.value.trim().toLowerCase() : '';
    var t = typeIn ? typeIn.value : 'all';

    function matchLogCategory(action, cat) {
      if (cat === 'all') return true;
      var a = String(action || '').toUpperCase();
      if (cat === 'AI') return a.indexOf('AI') >= 0 || a.indexOf('DETECTION') >= 0;
      if (cat === 'SENSOR') return a.indexOf('SENSOR') >= 0;
      if (cat === 'COMMAND') return a.indexOf('COMMAND') >= 0 || a.indexOf('DEVICE') >= 0;
      if (cat === 'ALERT') return a.indexOf('ALERT') >= 0;
      if (cat === 'ADMIN') return a.indexOf('USER') >= 0 || a.indexOf('CAMERA') >= 0 || a.indexOf('SETTINGS') >= 0;
      return true;
    }

    var filteredLogs = logItems.filter(function (l) {
      var mQ = !q || (l.action + ' ' + l.description).toLowerCase().indexOf(q) >= 0;
      return mQ && matchLogCategory(l.action, t);
    });

    var histTimeline = document.querySelector('[data-page="history"] .timeline');
    if (histTimeline) {
      histTimeline.replaceChildren();
      if (!filteredLogs.length) {
        histTimeline.innerHTML = '<div style="color:#64748b;padding:12px 0">Chưa có dòng thời gian sự kiện khớp bộ lọc.</div>';
      } else {
        filteredLogs.slice(0, 10).forEach(function (l) {
          var div = document.createElement('div');
          div.className = 'timeline-item';
          div.innerHTML = '<strong></strong><small></small>';
          div.querySelector('strong').textContent = l.action;
          div.querySelector('small').textContent = l.description + ' · ' + new Date(l.created_at).toLocaleString('vi-VN');
          histTimeline.appendChild(div);
        });
      }
    }

    var histBadge = document.getElementById('ss-hist-badge');
    if (histBadge) histBadge.textContent = filteredLogs.length + ' bản ghi';

    var histBody = document.querySelector('[data-page="history"] .table tbody');
    if (histBody) {
      histBody.replaceChildren();
      if (!filteredLogs.length) {
        histBody.appendChild(emptyRow(5, 'Chưa có lịch sử sự kiện khớp bộ lọc.'));
      } else {
        filteredLogs.forEach(function (l) {
          var tr = document.createElement('tr');
          tr.innerHTML = '<td></td><td><span class="status info"></span></td><td></td><td></td><td></td>';
          var tds = tr.querySelectorAll('td');
          tds[0].textContent = new Date(l.created_at).toLocaleString('vi-VN');
          tds[1].querySelector('span').textContent = l.action;
          tds[2].textContent = l.description;
          tds[3].textContent = l.user_id ? ('User #' + l.user_id) : 'Hệ thống / Tự động';
          tds[4].textContent = 'Log #' + l.id;
          histBody.appendChild(tr);
        });
      }
    }
  }

  /* ==================== 8. USERS ==================== */
  function renderUsersView() {
    var usersTbody = document.getElementById('ss-users-tbody');
    if (!usersTbody) return;
    var searchIn = document.getElementById('ss-user-search');
    var roleIn = document.getElementById('ss-user-role-filter');
    var q = searchIn ? searchIn.value.trim().toLowerCase() : '';
    var r = roleIn ? roleIn.value : 'all';

    var filtered = state.users.filter(function (u) {
      var mQ = !q || (u.full_name + ' ' + u.username + ' ' + u.role).toLowerCase().indexOf(q) >= 0;
      var mR = r === 'all' || u.role === r;
      return mQ && mR;
    });

    usersTbody.replaceChildren();
    if (!filtered.length) {
      usersTbody.appendChild(emptyRow(6, 'Không tìm thấy người dùng nào.'));
      return;
    }

    filtered.forEach(function (u) {
      var tr = document.createElement('tr');
      var isSelf = state.me && state.me.id === u.id;
      tr.innerHTML =
        '<td><div style="display:flex;align-items:center;gap:11px"><i class="avatar">' + initials(u.full_name) + '</i><div><strong></strong><small style="display:block;color:var(--ss-muted);font-size:11px">' + (u.role === 'ADMIN' ? 'Quản trị hệ thống' : 'Nhân viên trực ca') + '</small></div></div></td>' +
        '<td style="font-family:var(--ss-font-mono);font-weight:600"></td>' +
        '<td><span class="status ' + (u.role === 'ADMIN' ? 'info' : 'unknown') + '">' + u.role + '</span></td>' +
        '<td><span class="status ' + (u.is_active ? 'online' : 'offline') + '"><span class="dot ' + (u.is_active ? '' : 'red') + '"></span>' + (u.is_active ? 'Hoạt động' : 'Đã khóa') + '</span></td>' +
        '<td style="font-family:var(--ss-font-mono);font-size:12px"></td>' +
        '<td><div class="table-actions"></div></td>';
      var tds = tr.querySelectorAll('td');
      var actionsWrap = tds[5].querySelector('.table-actions');
      tds[0].querySelector('strong').textContent = u.full_name + (isSelf ? ' (Bạn)' : '');
      tds[1].textContent = u.username;
      tds[4].textContent = u.created_at ? new Date(u.created_at).toLocaleDateString('vi-VN') : '—';

      if (isAdmin() && !isSelf) {
        var roleBtn = document.createElement('button');
        roleBtn.className = 'btn subtle';
        var nextRole = u.role === 'ADMIN' ? 'OPERATOR' : 'ADMIN';
        roleBtn.textContent = 'Đổi → ' + nextRole;
        roleBtn.addEventListener('click', function () {
          request('/users/' + u.id, { method: 'PUT', body: JSON.stringify({ role: nextRole }) }).then(function () {
            toast('Đã đổi vai trò của ' + u.username + ' sang ' + nextRole);
            refreshDataOnly();
          }).catch(function (err) { toast(err.detail || 'Lỗi đổi vai trò'); });
        });

        var lockBtn = document.createElement('button');
        lockBtn.className = 'btn';
        lockBtn.textContent = u.is_active ? 'Khóa' : 'Mở khóa';
        lockBtn.addEventListener('click', function () {
          request('/users/' + u.id, { method: 'PUT', body: JSON.stringify({ is_active: !u.is_active }) }).then(function () {
            toast((u.is_active ? 'Đã khóa' : 'Đã mở khóa') + ' tài khoản ' + u.username);
            refreshDataOnly();
          }).catch(function (err) { toast(err.detail || 'Lỗi cập nhật trạng thái'); });
        });

        var delBtn = document.createElement('button');
        delBtn.className = 'btn danger';
        delBtn.textContent = 'Xóa';
        delBtn.addEventListener('click', function () {
          confirmAction('Xóa vĩnh viễn tài khoản "' + u.username + '"?', function () {
            request('/users/' + u.id, { method: 'DELETE' }).then(function () {
              toast('Đã xóa tài khoản ' + u.username);
              refreshDataOnly();
            }).catch(function (err) { toast(err.detail || 'Không thể xóa tài khoản'); });
          });
        });

        actionsWrap.appendChild(roleBtn);
        actionsWrap.appendChild(lockBtn);
        actionsWrap.appendChild(delBtn);
      } else {
        actionsWrap.innerHTML = isSelf
          ? '<span class="status info"><span class="dot"></span>Đang đăng nhập</span>'
          : '<span class="status unknown">Chỉ xem</span>';
      }
      usersTbody.appendChild(tr);
    });
  }

  /* ==================== 9 & 10. SYSTEM & SETTINGS ==================== */
  function renderSystemAndSettings() {
    var s = state.overview || {};
    var h = state.health || {};
    var sysUpd = document.getElementById('ss-sys-updated');
    if (sysUpd) sysUpd.textContent = 'Kiểm tra lúc ' + new Date().toLocaleTimeString('vi-VN');

    var sysApi = document.getElementById('ss-sys-api');
    var sysDb = document.getElementById('ss-sys-db');
    var sysVpn = document.getElementById('ss-sys-vpn');
    var sysCamSub = document.getElementById('ss-sys-cam-sub');
    var sysCam = document.getElementById('ss-sys-cam');
    var sysDevSub = document.getElementById('ss-sys-dev-sub');
    var sysDev = document.getElementById('ss-sys-dev');
    var sysAi = document.getElementById('ss-sys-ai');
    var sideVpnDot = document.getElementById('ss-side-vpn-dot');
    var sideVpnSub = document.getElementById('ss-side-vpn-sub');

    var apiOk = h.api === 'ok';
    var dbOk = h.database === 'ok';
    var vpnOk = h.wireguard_vpn === 'online' || h.vpn === 'online';
    var mqttOk = h.mqtt_broker === 'online' || h.mqtt === 'online';
    var camOnline = s.camera_online || 0;
    var camTotal = s.camera_total || 0;
    var devOnline = s.device_online || 0;
    var devTotal = s.device_total || 0;
    var aiOk = h.ai_worker === 'online' || h.ai === 'online' || state.detections.length > 0;

    if (sysApi) sysApi.innerHTML = apiOk ? '<span class="dot"></span>Đang hoạt động' : '<span class="dot offline"></span>Tạm thời chưa hoạt động';
    if (sysDb) sysDb.innerHTML = dbOk ? '<span class="dot"></span>Đang hoạt động' : '<span class="dot offline"></span>Tạm thời chưa hoạt động';
    if (sysVpn) sysVpn.innerHTML = vpnOk ? '<span class="dot"></span>Đang hoạt động' : '<span class="dot offline"></span>Tạm thời chưa hoạt động';
    if (sideVpnDot) sideVpnDot.className = 'dot ' + (vpnOk ? '' : 'offline');
    if (sideVpnSub) sideVpnSub.textContent = vpnOk ? 'Đường hầm bảo mật · Đang hoạt động' : 'Tạm thời chưa hoạt động';

    if (sysCamSub) sysCamSub.textContent = camOnline + ' / ' + camTotal + ' luồng trực tuyến';
    if (sysCam) sysCam.innerHTML = camOnline > 0
      ? ('<span class="dot"></span>Đang hoạt động (' + camOnline + ' camera)')
      : '<span class="dot offline"></span>Tạm thời chưa hoạt động';

    if (sysDevSub) sysDevSub.textContent = (mqttOk ? 'MQTT :1883 Online' : 'MQTT Offline') + ' · ' + devOnline + '/' + devTotal + ' ESP32';
    if (sysDev) sysDev.innerHTML = devOnline > 0
      ? ('<span class="dot"></span>Đang hoạt động (' + devOnline + ' thiết bị)')
      : '<span class="dot offline"></span>Tạm thời chưa hoạt động';

    if (sysAi) sysAi.innerHTML = aiOk ? '<span class="dot"></span>Đang hoạt động' : '<span class="dot offline"></span>Tạm thời chưa hoạt động';

    var cfg = state.settings || {};
    var mq2In = document.getElementById('ss-set-mq2');
    var confIn = document.getElementById('ss-set-conf');
    var dedupIn = document.getElementById('ss-set-dedup');
    var retIn = document.getElementById('ss-set-retention');
    if (mq2In && cfg.mq2_threshold != null) mq2In.value = cfg.mq2_threshold;
    if (confIn && cfg.alert_confidence_threshold != null) confIn.value = Math.round(cfg.alert_confidence_threshold * 100);
    if (dedupIn && cfg.alert_deduplication_seconds != null) dedupIn.value = cfg.alert_deduplication_seconds;
    if (retIn && cfg.retention_days != null) retIn.value = String(cfg.retention_days);

    var mq2Label = document.getElementById('ss-mq2-threshold-label');
    if (mq2Label && cfg.mq2_threshold != null) mq2Label.textContent = 'Ngưỡng cảnh báo: ' + cfg.mq2_threshold + ' ppm';
  }

  /* ==================== MODALS (CAMERA, USER, DEVICE, AI SIM, SENSOR SIM) ==================== */
  function openCameraFormModal(existingCam) {
    var isEdit = Boolean(existingCam);
    var camId = isEdit ? (existingCam.camera_id != null ? existingCam.camera_id : existingCam.id) : null;
    var backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML =
      '<div class="modal" role="dialog" aria-modal="true">' +
      '<div class="modal-head"><span>' +
        (isEdit
          ? ('Cập nhật Camera <span class="cam-id-badge" style="margin-left:8px">camera_id #' + camId + '</span>')
          : 'Thông tin Camera mới') +
      '</span><button class="modal-close" aria-label="Đóng">×</button></div>' +
      '<div class="modal-grid">' +
      '<div class="modal-field"><label for="ss-field-0">Tên Camera *</label><input id="ss-field-0" type="text" placeholder="Nhập tên camera (vd: CAM-01 · Nhà kho Hóa chất)"></div>' +
      '<div class="modal-field"><label for="ss-field-1">Địa chỉ IP</label><input id="ss-field-1" type="text" placeholder="Nhập địa chỉ IP (vd: 192.168.10.21)"></div>' +
      '<div class="modal-field full"><label for="ss-field-2">' +
        (isEdit ? 'RTSP URL mới (để trống để giữ nguyên URL gốc)' : 'RTSP URL *') +
      '</label><input id="ss-field-2" type="text" placeholder="' +
        (isEdit ? 'Để trống nếu muốn giữ nguyên RTSP URL gốc đã lưu...' : 'rtsp://user:pass@192.168.10.21:554/stream1') +
      '">' +
      (isEdit
        ? '<div class="rtsp-hint-box"><strong>Bảo toàn RTSP URL:</strong> Hệ thống đã che mật khẩu trên giao diện (<code data-cur-rtsp></code>). Hãy <b>để trống ô trên</b> nếu bạn chỉ sửa tên/vị trí/trạng thái — Backend sẽ giữ nguyên RTSP URL gốc kèm tài khoản/mật khẩu.</div>'
        : '<div class="rtsp-hint-box">Chỉ chấp nhận giao thức <code>rtsp://</code> hoặc <code>rtsps://</code> (hoặc luồng MediaMTX <code>rtsp://mediamtx:8554/...</code>). Mật khẩu được che tự động trên giao diện và chỉ cấp nội bộ cho AI Service.</div>') +
      '</div>' +
      '<div class="modal-field"><label for="ss-field-3">Khu vực lắp đặt PCCC</label><input id="ss-field-3" type="text" placeholder="Nhập khu vực (vd: Nhà kho Khu A / Tầng hầm B1)"></div>' +
      '<div class="modal-field"><label for="ss-field-4">Trạng thái</label><select id="ss-field-4" class="select" style="width:100%"><option value="OFFLINE">OFFLINE (Tạm thời chưa hoạt động)</option><option value="ONLINE">ONLINE (Trực tuyến)</option></select></div>' +
      '</div>' +
      '<div class="modal-foot"><button class="btn" data-cancel>Hủy</button><button class="btn primary" data-save>Lưu thông tin</button></div>' +
      '</div>';
    if (isEdit) {
      backdrop.querySelector('#ss-field-0').value = existingCam.name || '';
      backdrop.querySelector('#ss-field-1').value = existingCam.ip_address || '';
      // Giữ trống ô RTSP khi sửa để không ghi đè bản URL đã che mật khẩu lên URL gốc trong DB
      backdrop.querySelector('#ss-field-2').value = '';
      backdrop.querySelector('#ss-field-3').value = existingCam.location || '';
      backdrop.querySelector('#ss-field-4').value = existingCam.status === 'ONLINE' ? 'ONLINE' : 'OFFLINE';
      var curRtspEl = backdrop.querySelector('[data-cur-rtsp]');
      if (curRtspEl) curRtspEl.textContent = existingCam.rtsp_url || 'chưa có';
    }
    document.getElementById('ss-modal-root').appendChild(backdrop);
    function close() { backdrop.remove(); }
    backdrop.querySelector('.modal-close').addEventListener('click', close);
    backdrop.querySelector('[data-cancel]').addEventListener('click', close);
    backdrop.addEventListener('click', function (e) { if (e.target === backdrop) close(); });
    backdrop.querySelector('[data-save]').addEventListener('click', function () {
      var rtspVal = backdrop.querySelector('#ss-field-2').value.trim();
      var payload = {
        name: backdrop.querySelector('#ss-field-0').value.trim(),
        ip_address: backdrop.querySelector('#ss-field-1').value.trim() || null,
        rtsp_url: isEdit ? (rtspVal || null) : rtspVal,
        location: backdrop.querySelector('#ss-field-3').value.trim() || null,
        status: backdrop.querySelector('#ss-field-4').value
      };
      if (!payload.name || (!isEdit && !rtspVal)) {
        toast('Vui lòng nhập Tên Camera và RTSP URL.');
        return;
      }
      if (rtspVal && !/^rtsps?:\/\/.+/i.test(rtspVal)) {
        toast('RTSP URL phải bắt đầu bằng rtsp:// hoặc rtsps://');
        return;
      }
      var method = isEdit ? 'PUT' : 'POST';
      var path = isEdit ? ('/cameras/' + camId) : '/cameras';
      request(path, { method: method, body: JSON.stringify(payload) }).then(function (saved) {
        close();
        state.monitorCameras = null;
        state.mgmtCameras = null;
        if (isEdit) {
          toast(saved && saved.rtsp_url_changed
            ? ('Đã cập nhật camera #' + camId + ' cùng RTSP URL mới.')
            : ('Đã cập nhật camera #' + camId + ' (giữ nguyên RTSP URL gốc).'));
        } else {
          toast('Đã thêm camera mới (camera_id #' + (saved && saved.id) + ').');
        }
        refreshDataOnly();
      }).catch(function (err) {
        toast(err.detail || 'Không thể lưu camera.');
      });
    });
  }

  function openUserFormModal() {
    var backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML =
      '<div class="modal" role="dialog" aria-modal="true">' +
      '<div class="modal-head">Thêm tài khoản người dùng<button class="modal-close" aria-label="Đóng">×</button></div>' +
      '<div class="modal-grid">' +
      '<div class="modal-field"><label for="ss-field-0">Họ và tên *</label><input id="ss-field-0" type="text" placeholder="Nhập họ và tên (vd: Nguyễn Văn A)"></div>' +
      '<div class="modal-field"><label for="ss-field-1">Tên đăng nhập (Username) *</label><input id="ss-field-1" type="text" placeholder="Nhập username (vd: operator1)"></div>' +
      '<div class="modal-field"><label for="ss-field-2">Mật khẩu (tối thiểu 8 ký tự) *</label><input id="ss-field-2" type="password" placeholder="Nhập mật khẩu"></div>' +
      '<div class="modal-field"><label for="ss-field-3">Vai trò (Role)</label><select id="ss-field-3" class="select" style="width:100%"><option value="OPERATOR">OPERATOR (Nhân viên vận hành)</option><option value="ADMIN">ADMIN (Quản trị viên)</option></select></div>' +
      '</div>' +
      '<div class="modal-foot"><button class="btn" data-cancel>Hủy</button><button class="btn primary" data-save>Lưu thông tin</button></div>' +
      '</div>';
    document.getElementById('ss-modal-root').appendChild(backdrop);
    function close() { backdrop.remove(); }
    backdrop.querySelector('.modal-close').addEventListener('click', close);
    backdrop.querySelector('[data-cancel]').addEventListener('click', close);
    backdrop.addEventListener('click', function (e) { if (e.target === backdrop) close(); });
    backdrop.querySelector('[data-save]').addEventListener('click', function () {
      var fullName = backdrop.querySelector('#ss-field-0').value.trim();
      var username = backdrop.querySelector('#ss-field-1').value.trim();
      var password = backdrop.querySelector('#ss-field-2').value;
      var role = backdrop.querySelector('#ss-field-3').value;
      if (!fullName || !username || password.length < 8) {
        toast('Vui lòng nhập đủ Họ tên, Username và Mật khẩu (tối thiểu 8 ký tự).');
        return;
      }
      request('/users', {
        method: 'POST',
        body: JSON.stringify({ full_name: fullName, username: username, password: password, role: role, is_active: true })
      }).then(function () {
        close();
        toast('Đã thêm tài khoản mới thành công.');
        refreshDataOnly();
      }).catch(function (err) {
        toast(err.detail || 'Không thể tạo tài khoản.');
      });
    });
  }

  function openDeviceFormModal() {
    var backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML =
      '<div class="modal" role="dialog" aria-modal="true">' +
      '<div class="modal-head">Thông tin thiết bị IoT<button class="modal-close" aria-label="Đóng">×</button></div>' +
      '<div class="modal-grid">' +
      '<div class="modal-field"><label for="ss-field-0">Mã thiết bị (Device Code) *</label><input id="ss-field-0" type="text" placeholder="Nhập mã thiết bị (vd: ESP32_001)"></div>' +
      '<div class="modal-field"><label for="ss-field-1">Tên thiết bị / Khu vực *</label><input id="ss-field-1" type="text" placeholder="Nhập tên thiết bị (vd: ESP32 Khu vực A)"></div>' +
      '<div class="modal-field"><label for="ss-field-2">Loại thiết bị</label><input id="ss-field-2" type="text" value="ESP32" placeholder="ESP32"></div>' +
      '<div class="modal-field"><label for="ss-field-3">Trạng thái ban đầu</label><select id="ss-field-3" class="select" style="width:100%"><option value="OFFLINE">OFFLINE (Tạm thời chưa hoạt động)</option><option value="ONLINE">ONLINE (Trực tuyến)</option></select></div>' +
      '</div>' +
      '<div class="modal-foot"><button class="btn" data-cancel>Hủy</button><button class="btn primary" data-save>Lưu thông tin</button></div>' +
      '</div>';
    document.getElementById('ss-modal-root').appendChild(backdrop);
    function close() { backdrop.remove(); }
    backdrop.querySelector('.modal-close').addEventListener('click', close);
    backdrop.querySelector('[data-cancel]').addEventListener('click', close);
    backdrop.addEventListener('click', function (e) { if (e.target === backdrop) close(); });
    backdrop.querySelector('[data-save]').addEventListener('click', function () {
      var code = backdrop.querySelector('#ss-field-0').value.trim();
      var name = backdrop.querySelector('#ss-field-1').value.trim();
      var type = backdrop.querySelector('#ss-field-2').value.trim() || 'ESP32';
      var status = backdrop.querySelector('#ss-field-3').value || 'ONLINE';
      if (!code || !name) {
        toast('Vui lòng nhập Mã thiết bị và Tên thiết bị.');
        return;
      }
      request('/devices', {
        method: 'POST',
        body: JSON.stringify({ device_code: code, device_name: name, device_type: type, status: status })
      }).then(function () {
        close();
        state.selectedDeviceCode = code;
        toast('Đã thêm thiết bị IoT ' + code + ' thành công.');
        refreshDataOnly();
      }).catch(function (err) {
        toast(err.detail || 'Không thể thêm thiết bị.');
      });
    });
  }

  function openAiSimulateModal() {
    if (!state.cameras.length) {
      toast('Vui lòng thêm ít nhất 1 Camera trước khi gửi sự kiện AI.');
      return;
    }
    var backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    var camOpts = state.cameras.map(function (c) {
      var cid = c.camera_id != null ? c.camera_id : c.id;
      return '<option value="' + cid + '">#' + cid + ' · ' + c.name + '</option>';
    }).join('');
    backdrop.innerHTML =
      '<div class="modal" role="dialog" aria-modal="true">' +
      '<div class="modal-head">Gửi sự kiện nhận diện Lửa &amp; Khói (YOLOv8 PCCC)<button class="modal-close" aria-label="Đóng">×</button></div>' +
      '<div class="modal-grid">' +
      '<div class="modal-field"><label for="ss-sim-cam">Chọn Camera PCCC</label><select id="ss-sim-cam" class="select" style="width:100%">' + camOpts + '</select></div>' +
      '<div class="modal-field"><label for="ss-sim-obj">Đối tượng YOLOv8 (Class)</label><select id="ss-sim-obj" class="select" style="width:100%"><option value="fire">fire (Lửa / Đám cháy)</option><option value="smoke">smoke (Khói)</option></select></div>' +
      '<div class="modal-field"><label for="ss-sim-evt">Loại sự kiện PCCC</label><select id="ss-sim-evt" class="select" style="width:100%"><option value="fire">fire (Báo cháy khẩn cấp -> CRITICAL)</option><option value="smoke">smoke (Cảnh báo khói -> WARNING / CRITICAL)</option></select></div>' +
      '<div class="modal-field"><label for="ss-sim-conf">Độ tin cậy (0.10 – 0.99)</label><input id="ss-sim-conf" type="number" step="0.01" min="0.1" max="0.99" value="0.95"></div>' +
      '<div class="modal-field full"><label for="ss-sim-file">Ảnh snapshot đính kèm (tùy chọn — JPEG/PNG/WebP)</label><input id="ss-sim-file" type="file" accept="image/jpeg,image/png,image/webp" style="padding-top:7px"></div>' +
      '</div>' +
      '<div class="modal-foot"><button class="btn" data-cancel>Hủy</button><button class="btn primary" data-save>Gửi sự kiện</button></div>' +
      '</div>';
    document.getElementById('ss-modal-root').appendChild(backdrop);
    function close() { backdrop.remove(); }
    backdrop.querySelector('.modal-close').addEventListener('click', close);
    backdrop.querySelector('[data-cancel]').addEventListener('click', close);
    backdrop.addEventListener('click', function (e) { if (e.target === backdrop) close(); });
    backdrop.querySelector('[data-save]').addEventListener('click', function () {
      var camId = parseInt(backdrop.querySelector('#ss-sim-cam').value, 10);
      var obj = backdrop.querySelector('#ss-sim-obj').value;
      var evt = backdrop.querySelector('#ss-sim-evt').value;
      var conf = parseFloat(backdrop.querySelector('#ss-sim-conf').value) || 0.92;
      var fileInput = backdrop.querySelector('#ss-sim-file');
      var file = fileInput && fileInput.files && fileInput.files[0];

      var uploadPromise = Promise.resolve(null);
      if (file) {
        var fd = new FormData();
        fd.append('camera_id', String(camId));
        fd.append('file', file);
        uploadPromise = requestMultipart('/detections/snapshot', fd).then(function (up) {
          return up && up.image_path ? up.image_path : null;
        });
      }

      uploadPromise.then(function (imagePath) {
        return request('/detections', {
          method: 'POST',
          body: JSON.stringify({
            camera_id: camId,
            object_type: obj,
            event_type: evt,
            confidence: conf,
            image_path: imagePath
          })
        });
      }).then(function (res) {
        close();
        state.filteredDetections = null;
        toast(res.alert_id ? ('Đã ghi nhận AI & tạo Cảnh báo ' + res.alert_severity + '!') : 'Đã ghi nhận sự kiện AI.');
        refreshDataOnly();
      }).catch(function (err) {
        toast(err.detail || 'Lỗi gửi sự kiện AI');
      });
    });
  }

  function openSensorSimulateModal() {
    var active = getActiveDevice();
    if (!active) {
      toast('Vui lòng thêm thiết bị IoT trước khi cập nhật cảm biến.');
      return;
    }
    var backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML =
      '<div class="modal" role="dialog" aria-modal="true">' +
      '<div class="modal-head">Cập nhật cảm biến (' + active.device_code + ')<button class="modal-close" aria-label="Đóng">×</button></div>' +
      '<div class="modal-grid">' +
      '<div class="modal-field"><label for="ss-sen-pir">Cảm biến Lửa (Flame IR · GPIO 27)</label><select id="ss-sen-pir" class="select" style="width:100%"><option value="false">An toàn (Không có tia lửa)</option><option value="true">PHÁT HIỆN TIA LỬA (Flame = TRUE)</option></select></div>' +
      '<div class="modal-field"><label for="ss-sen-mq2">Nồng độ khói / khí gas MQ-2 (ppm)</label><input id="ss-sen-mq2" type="number" min="0" max="10000" value="' + (active.latest_mq2 != null ? active.latest_mq2 : 340) + '"></div>' +
      '</div>' +
      '<div class="modal-foot"><button class="btn" data-cancel>Hủy</button><button class="btn primary" data-save>Cập nhật</button></div>' +
      '</div>';
    backdrop.querySelector('#ss-sen-pir').value = active.latest_pir ? 'true' : 'false';
    document.getElementById('ss-modal-root').appendChild(backdrop);
    function close() { backdrop.remove(); }
    backdrop.querySelector('.modal-close').addEventListener('click', close);
    backdrop.querySelector('[data-cancel]').addEventListener('click', close);
    backdrop.addEventListener('click', function (e) { if (e.target === backdrop) close(); });
    backdrop.querySelector('[data-save]').addEventListener('click', function () {
      var pir = backdrop.querySelector('#ss-sen-pir').value === 'true';
      var mq2 = parseFloat(backdrop.querySelector('#ss-sen-mq2').value) || 0;
      request('/sensors/data', {
        method: 'POST',
        body: JSON.stringify({ device_id: active.device_code, pir: pir, mq2: mq2, relay: active.relay, buzzer: active.buzzer, lock: active.lock_engaged })
      }).then(function () {
        close();
        toast('Đã cập nhật thông số cảm biến cho ' + active.device_code);
        refreshDataOnly();
      }).catch(function (err) {
        toast(err.detail || 'Lỗi cập nhật cảm biến');
      });
    });
  }

  /* ==================== CSV EXPORT ==================== */
  function exportCsv(filename, headers, rows) {
    var lines = [headers.join(',')];
    rows.forEach(function (r) {
      lines.push(r.map(function (c) { return '"' + String(c == null ? '' : c).replace(/"/g, '""') + '"'; }).join(','));
    });
    var blob = new Blob(['\uFEFF' + lines.join('\n')], { type: 'text/csv;charset=utf-8;' });
    var url = URL.createObjectURL(blob);
    var a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    toast('Đã xuất file ' + filename);
  }

  /* ==================== EVENT BINDINGS ==================== */
  function bindInteractiveControls() {
    var logoutBtn = document.getElementById('ss-logout-btn');
    if (logoutBtn) logoutBtn.addEventListener('click', logout);

    document.querySelectorAll('[data-action="refresh"]').forEach(function (btn) {
      btn.addEventListener('click', function () {
        btn.classList.remove('is-spinning');
        void btn.offsetWidth;
        btn.classList.add('is-spinning');
        setTimeout(function () {
          btn.classList.remove('is-spinning');
        }, 650);
        refreshDataOnly();
        toast('Đã làm mới dữ liệu thực tế từ máy chủ.');
      });
    });

    document.querySelectorAll('[data-modal]').forEach(function (btn) {
      btn.addEventListener('click', function () {
        var kind = btn.dataset.modal;
        if (kind === 'camera') openCameraFormModal(null);
        else if (kind === 'user') openUserFormModal();
        else if (kind === 'device') openDeviceFormModal();
      });
    });

    var simAiBtn = document.getElementById('ss-btn-sim-ai');
    if (simAiBtn) simAiBtn.addEventListener('click', openAiSimulateModal);

    var simSenBtn = document.getElementById('ss-btn-sim-sensor');
    if (simSenBtn) simSenBtn.addEventListener('click', openSensorSimulateModal);

    var daysSelect = document.getElementById('ss-overview-days');
    if (daysSelect) {
      daysSelect.addEventListener('change', function () {
        var d = parseInt(daysSelect.value, 10) || 7;
        request('/statistics/detections?days=' + d).then(function (res) {
          state.detStats = res.items || [];
          renderOverviewDetections();
          renderOverviewBars();
        });
      });
    }

    // Monitor & Cameras filters (gọi API GET /api/cameras?search=&location=&status= có debounce)
    function scheduleCameraFilter(scope) {
      clearTimeout(camFilterTimer);
      camFilterTimer = setTimeout(function () {
        fetchFilteredCameras(scope);
      }, 220);
    }
    ['[data-camera-search]', '[data-camera-region]', '[data-camera-status]'].forEach(function (sel) {
      var el = document.querySelector(sel);
      if (el) {
        el.addEventListener('input', function () { scheduleCameraFilter('monitor'); });
        el.addEventListener('change', function () { fetchFilteredCameras('monitor'); });
      }
    });
    ['#ss-cam-mgmt-search', '#ss-cam-mgmt-location', '#ss-cam-mgmt-status'].forEach(function (sel) {
      var el = document.querySelector(sel);
      if (el) {
        el.addEventListener('input', function () { scheduleCameraFilter('mgmt'); });
        el.addEventListener('change', function () { fetchFilteredCameras('mgmt'); });
      }
    });

    // AI filters (gọi API GET /api/detections?camera_id=&object_type=&event_type=&min_confidence=)
    document.querySelectorAll('#ss-ai-filters .filter-chip').forEach(function (chip) {
      chip.addEventListener('click', function () {
        document.querySelectorAll('#ss-ai-filters .filter-chip').forEach(function (c) { c.classList.remove('active'); });
        chip.classList.add('active');
        state.aiChipFilter = chip.dataset.aiFilter || 'all';
        fetchFilteredDetections();
      });
    });
    ['#ss-ai-camera-filter', '#ss-ai-event-filter', '#ss-ai-conf-filter'].forEach(function (sel) {
      var el = document.querySelector(sel);
      if (el) {
        el.addEventListener('change', fetchFilteredDetections);
      }
    });
    var aiSearch = document.getElementById('ss-ai-search');
    if (aiSearch) {
      aiSearch.addEventListener('input', function () {
        clearTimeout(aiFilterTimer);
        aiFilterTimer = setTimeout(renderAiView, 120);
      });
    }

    // IoT controls
    var devSelect = document.getElementById('ss-iot-device-select');
    if (devSelect) {
      devSelect.addEventListener('change', function () {
        state.selectedDeviceCode = devSelect.value;
        renderIotView();
      });
    }

    function sendDeviceCommand(action, desc) {
      var active = getActiveDevice();
      if (!active || (active.status !== 'ONLINE' && active.latest_pir === null && active.latest_mq2 === null)) {
        toast('Tạm thời chưa hoạt động: Thiết bị chưa kết nối hoặc đang ngoại tuyến.');
        return;
      }
      confirmAction('Bạn có muốn gửi lệnh "' + desc + '" tới thiết bị ' + active.device_code + '?', function () {
        request('/devices/' + encodeURIComponent(active.device_code) + '/command', {
          method: 'POST',
          body: JSON.stringify({ action: action })
        }).then(function (res) {
          toast(res.message || ('Đã thực thi lệnh ' + desc + ' trên ' + active.device_code));
          refreshDataOnly();
        }).catch(function (err) {
          toast(err.detail || 'Lỗi gửi lệnh điều khiển');
        });
      });
    }

    var toggleRelay = document.getElementById('ss-toggle-relay');
    if (toggleRelay) {
      toggleRelay.addEventListener('click', function () {
        var active = getActiveDevice();
        var nextAction = (active && active.relay) ? 'relay_off' : 'relay_on';
        sendDeviceCommand(nextAction, nextAction === 'relay_on' ? 'KÍCH HOẠT Mạch Relay bơm chữa cháy / phun sương' : 'NGẮT Mạch Relay bơm chữa cháy');
      });
    }

    var toggleBuzzer = document.getElementById('ss-toggle-buzzer');
    if (toggleBuzzer) {
      toggleBuzzer.addEventListener('click', function () {
        var active = getActiveDevice();
        var nextAction = (active && active.buzzer) ? 'buzzer_off' : 'buzzer_on';
        sendDeviceCommand(nextAction, nextAction === 'buzzer_on' ? 'BẬT Còi báo động 12V' : 'TẮT Còi báo động 12V');
      });
    }

    var btnLock = document.getElementById('ss-btn-lock-toggle');
    if (btnLock) {
      btnLock.addEventListener('click', function () {
        var active = getActiveDevice();
        var isLocked = !active || active.lock_engaged !== false;
        var nextAction = isLocked ? 'unlock' : 'lock';
        sendDeviceCommand(nextAction, isLocked ? 'NHẢ Khóa từ Solenoid mở cửa thoát hiểm khẩn cấp' : 'KHÓA LẠI Khóa từ Solenoid cửa thoát hiểm');
      });
    }

    // Alerts filters & export
    ['#ss-alert-search', '#ss-alert-severity', '#ss-alert-status'].forEach(function (sel) {
      var el = document.querySelector(sel);
      if (el) {
        el.addEventListener('input', renderAlertsView);
        el.addEventListener('change', renderAlertsView);
      }
    });
    var alertReset = document.getElementById('ss-alert-reset');
    if (alertReset) {
      alertReset.addEventListener('click', function () {
        var sIn = document.getElementById('ss-alert-search');
        var sevIn = document.getElementById('ss-alert-severity');
        var stIn = document.getElementById('ss-alert-status');
        if (sIn) sIn.value = '';
        if (sevIn) sevIn.value = 'all';
        if (stIn) stIn.value = 'all';
        renderAlertsView();
        toast('Đã đặt lại bộ lọc cảnh báo.');
      });
    }
    var checkAll = document.getElementById('ss-alert-check-all');
    if (checkAll) {
      checkAll.addEventListener('change', function () {
        document.querySelectorAll('.ss-alert-row-check').forEach(function (cb) {
          cb.checked = checkAll.checked;
        });
      });
    }
    var alertExport = document.getElementById('ss-alert-export');
    if (alertExport) {
      alertExport.addEventListener('click', function () {
        exportCsv(
          'smartsecure_alerts.csv',
          ['ID', 'Description', 'Type', 'Severity', 'Status', 'Created At'],
          state.alerts.map(function (a) { return ['#ALT-' + String(a.id).padStart(4, '0'), a.description, a.alert_type, a.severity, a.status, a.created_at]; })
        );
      });
    }

    // History filters & export
    ['#ss-hist-search', '#ss-hist-type'].forEach(function (sel) {
      var el = document.querySelector(sel);
      if (el) {
        el.addEventListener('input', renderHistoryView);
        el.addEventListener('change', renderHistoryView);
      }
    });
    var histExport = document.getElementById('ss-hist-export');
    if (histExport) {
      histExport.addEventListener('click', function () {
        exportCsv(
          'smartsecure_history.csv',
          ['ID', 'Action', 'Description', 'User ID', 'Created At'],
          state.logs.map(function (l) { return [l.id, l.action, l.description, l.user_id || 'System', l.created_at]; })
        );
      });
    }
    var histClear = document.getElementById('ss-hist-clear');
    if (histClear) {
      histClear.addEventListener('click', function () {
        confirmAction('Bạn có chắc muốn xóa toàn bộ nhật ký lịch sử sự kiện?', function () {
          request('/logs/system', { method: 'DELETE' }).then(function () {
            toast('Đã xóa toàn bộ lịch sử sự kiện.');
            refreshDataOnly();
          }).catch(function (err) {
            toast(err.detail || 'Không thể xóa lịch sử.');
          });
        });
      });
    }

    // Users filters
    ['#ss-user-search', '#ss-user-role-filter'].forEach(function (sel) {
      var el = document.querySelector(sel);
      if (el) {
        el.addEventListener('input', renderUsersView);
        el.addEventListener('change', renderUsersView);
      }
    });

    // Global search mirrors into current active view's search input
    var globalSearch = document.getElementById('ss-global-search');
    if (globalSearch) {
      globalSearch.addEventListener('input', function () {
        var val = globalSearch.value;
        ['[data-camera-search]', '#ss-ai-search', '#ss-alert-search', '#ss-hist-search', '#ss-cam-mgmt-search', '#ss-user-search'].forEach(function (sel) {
          var el = document.querySelector(sel);
          if (el) el.value = val;
        });
        renderCamerasViews();
        renderAiView();
        renderAlertsView();
        renderHistoryView();
        renderUsersView();
      });
    }

    // Save Settings
    var saveSetBtn = document.getElementById('ss-save-settings');
    if (saveSetBtn) {
      saveSetBtn.addEventListener('click', function () {
        var mq2 = parseFloat(document.getElementById('ss-set-mq2').value) || 600;
        var confPct = parseFloat(document.getElementById('ss-set-conf').value) || 80;
        var dedup = parseInt(document.getElementById('ss-set-dedup').value, 10) || 60;
        var retention = parseInt(document.getElementById('ss-set-retention').value, 10) || 30;
        request('/system/settings', {
          method: 'PUT',
          body: JSON.stringify({
            mq2_threshold: mq2,
            alert_confidence_threshold: Math.min(1, Math.max(0.1, confPct / 100)),
            alert_deduplication_seconds: dedup,
            retention_days: retention
          })
        }).then(function (updated) {
          state.settings = updated;
          renderSystemAndSettings();
          toast('Đã lưu cài đặt hệ thống vào máy chủ.');
        }).catch(function (err) {
          toast(err.detail || 'Không thể lưu cài đặt.');
        });
      });
    }
  }

  function connectWebSocket() {
    if (!window.WebSocket || !token || activeSocket) return;
    if (wsReconnectTimer) {
      clearTimeout(wsReconnectTimer);
      wsReconnectTimer = null;
    }
    var socketUrl = (location.protocol === 'https:' ? 'wss://' : 'ws://') + location.host + '/ws/dashboard?token=' + encodeURIComponent(token);
    activeSocket = new WebSocket(socketUrl);
    activeSocket.onopen = function () {
      var rtNote = document.getElementById('ss-ai-realtime-note');
      if (rtNote) rtNote.textContent = 'WebSocket Realtime: Đã kết nối (detection.created)';
    };
    activeSocket.onmessage = function (ev) {
      try {
        var msg = JSON.parse(ev.data);
        if (msg && msg.type === 'detection.created' && msg.data) {
          var d = msg.data;
          var pct = d.confidence != null ? (Number(d.confidence) * 100).toFixed(1) + '%' : '';
          var camLabel = d.camera_name || ('Camera #' + d.camera_id);
          toast('AI phát hiện ' + d.object_type + (pct ? ' (' + pct + ')' : '') + ' tại ' + camLabel);
          // Đưa sự kiện mới lên đầu danh sách ngay lập tức trước khi đồng bộ đầy đủ
          var exists = state.detections.some(function (x) { return x.id === d.id; });
          if (!exists && d.id != null) {
            state.detections.unshift({
              id: d.id,
              camera_id: d.camera_id,
              camera_name: d.camera_name || camLabel,
              object_type: d.object_type,
              confidence: d.confidence,
              event_type: d.event_type,
              image_path: d.image_path || null,
              timestamp: d.timestamp || new Date().toISOString()
            });
            state.filteredDetections = null;
            renderAiView();
          }
          var rtNote = document.getElementById('ss-ai-realtime-note');
          if (rtNote) rtNote.textContent = 'Nhận detection.created #' + d.id + ' lúc ' + new Date().toLocaleTimeString('vi-VN');
        }
      } catch (e) { /* ignore non-JSON frames */ }
      refreshDataOnly();
    };
    activeSocket.onclose = function () {
      activeSocket = null;
      var rtNote = document.getElementById('ss-ai-realtime-note');
      if (rtNote) rtNote.textContent = 'WebSocket: Đang kết nối lại...';
      if (token) {
        wsReconnectTimer = setTimeout(connectWebSocket, 3000);
      }
    };
  }

  function refreshDataOnly() {
    if (!token) return loginDialog();
    var daysSelect = document.getElementById('ss-overview-days');
    var days = daysSelect ? (parseInt(daysSelect.value, 10) || 7) : 7;

    Promise.all([
      request('/auth/me'),
      request('/statistics/overview'),
      request('/alerts?limit=100'),
      request('/statistics/detections?days=' + days),
      request('/cameras?limit=100'),
      request('/devices'),
      request('/logs/system?limit=100'),
      request('/detections?limit=100'),
      request('/users').catch(function (err) {
        if (err && err.status === 403) return { items: [] };
        throw err;
      }),
      request('/system/health'),
      request('/system/settings')
    ]).then(function (results) {
      applyUserRole(results[0]);
      state.overview = results[1] || {};
      state.alerts = (results[2] && results[2].items) || [];
      state.detStats = (results[3] && results[3].items) || [];
      state.cameras = (results[4] && results[4].items) || [];
      state.cameraLocations = (results[4] && Array.isArray(results[4].locations)) ? results[4].locations : [];
      state.monitorCameras = null;
      state.mgmtCameras = null;
      state.devices = (results[5] && results[5].items) || [];
      state.logs = (results[6] && results[6].items) || [];
      state.detections = (results[7] && results[7].items) || [];
      state.filteredDetections = null;
      state.users = (results[8] && results[8].items) || [];
      state.health = results[9] || {};
      state.settings = results[10] || {};

      renderOverviewStats(state.overview);
      renderOverviewBars();
      renderOverviewAlerts();
      renderOverviewDetections();
      renderOverviewDevices();
      renderOverviewActivity();
      renderQuickCameras();
      renderCamerasViews();
      renderAiView();
      renderIotView();
      renderAlertsView();
      renderHistoryView();
      renderUsersView();
      renderSystemAndSettings();
    }).catch(function (error) {
      if (error.status === 401 || error.message === 'API 401') {
        logout();
        return;
      }
      var headerPill = document.querySelector('.header-system');
      if (headerPill) {
        headerPill.innerHTML = '<i class="dot red"></i>Mất kết nối API';
      }
      var sysApi = document.getElementById('ss-sys-api');
      if (sysApi) {
        sysApi.innerHTML = '<i class="dot offline"></i>Tạm thời chưa hoạt động';
      }
    });
  }

  function refresh() {
    refreshDataOnly();
    connectWebSocket();
  }

  window.smartSecureRefreshOverview = refreshDataOnly;

  function initTheme() {
    var rootEl = document.getElementById('smartsecure-prototype');
    var themeBtn = document.getElementById('ss-theme-toggle');
    if (!rootEl) return;
    var savedTheme = localStorage.getItem('smartsecure_theme') || 'light';
    if (savedTheme === 'dark') {
      rootEl.setAttribute('data-theme', 'dark');
    }
    if (themeBtn) {
      themeBtn.addEventListener('click', function () {
        var isDark = rootEl.getAttribute('data-theme') === 'dark';
        if (isDark) {
          rootEl.removeAttribute('data-theme');
          localStorage.setItem('smartsecure_theme', 'light');
          toast('Đã chuyển sang giao diện Enterprise Light.');
        } else {
          rootEl.setAttribute('data-theme', 'dark');
          localStorage.setItem('smartsecure_theme', 'dark');
          toast('Đã chuyển sang giao diện Cyber Dark SOC.');
        }
      });
    }
  }

  function init() {
    initTheme();
    bindInteractiveControls();
    refresh();
    setInterval(function () {
      if (token && !document.hidden) {
        refreshDataOnly();
      }
    }, 6000);
  }

  if (document.readyState === 'loading') {
    window.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
