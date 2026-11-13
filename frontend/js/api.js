(function () {
  'use strict';
  var token = sessionStorage.getItem('smartsecure_token');
  var apiRoot = '/api';
  var activeSocket = null;

  var state = {
    me: null,
    overview: {},
    cameras: [],
    devices: [],
    selectedDeviceCode: null,
    alerts: [],
    detections: [],
    detStats: [],
    alertStats: [],
    logs: [],
    users: [],
    health: {},
    settings: {},
    aiChipFilter: 'all'
  };

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
    if (greeting) greeting.textContent = 'Xin chào, ' + me.full_name + ' (' + roleText + '). Đây là tình hình an ninh hôm nay.';

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
      '<div class="brand"><div class="brandmark">✓</div><div>SmartSecure<small>ĐĂNG NHẬP HỆ THỐNG</small></div></div>' +
      '<form>' +
      '<label>Tên đăng nhập<input name="username" autocomplete="username" required placeholder="Nhập username (vd: admin)"></label>' +
      '<label>Mật khẩu<input name="password" type="password" autocomplete="current-password" required placeholder="Nhập mật khẩu"></label>' +
      '<p role="alert"></p>' +
      '<button class="btn primary" type="submit">Đăng nhập</button>' +
      '<small>Hỗ trợ phân quyền Quản trị viên (ADMIN) và Nhân viên vận hành (OPERATOR).</small>' +
      '</form></div>';
    document.body.appendChild(box);
    box.querySelector('form').addEventListener('submit', function (event) {
      event.preventDefault();
      var form = new FormData(event.currentTarget);
      var alertEl = box.querySelector('[role=alert]');
      alertEl.textContent = '';
      request('/auth/login', {
        method: 'POST',
        body: JSON.stringify({ username: String(form.get('username') || '').trim(), password: String(form.get('password') || '') })
      }).then(function (data) {
        token = data.access_token;
        sessionStorage.setItem('smartsecure_token', token);
        box.remove();
        refresh();
      }).catch(function (err) {
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
    if (center) center.innerHTML = total + '<small>đối tượng</small>';
    var aiNote = document.getElementById('ss-overview-ai-note');
    if (aiNote) aiNote.textContent = total + ' lượt nhận diện';
    var donut = document.getElementById('ss-overview-donut');

    var legendList = overview.querySelector('.donut-wrap .legend-list');
    if (!legendList) return;
    legendList.replaceChildren();
    if (total <= 0) {
      if (donut) donut.style.background = '#e2e8f0';
      var empty = document.createElement('div');
      empty.className = 'legend-row';
      empty.textContent = 'Chưa có dữ liệu nhận diện';
      legendList.appendChild(empty);
      return;
    }
    var colors = ['#2563eb', '#f59e0b', '#10b981', '#8b5cf6'];
    var stops = [];
    var currentDeg = 0;
    var labelMap = { person: 'Người', car: 'Xe ô tô', truck: 'Xe tải', vehicle: 'Phương tiện', animal: 'Động vật', fire: 'Lửa / Cháy', smoke: 'Khói', weapon: 'Vũ khí' };
    items.slice(0, 4).forEach(function (item, idx) {
      var pct = Math.round((item.count / total) * 100);
      var nextDeg = idx === items.slice(0, 4).length - 1 ? 100 : (currentDeg + pct);
      stops.push(colors[idx % colors.length] + ' ' + currentDeg + '% ' + nextDeg + '%');
      currentDeg = nextDeg;

      var row = document.createElement('div');
      row.className = 'legend-row';
      var dot = document.createElement('i');
      dot.style.background = colors[idx % colors.length];
      var label = document.createTextNode(labelMap[item.object_type] || item.object_type);
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
        ? '<span class="quick-preview"><i class="quick-person"></i><b class="live-tag">● LIVE</b></span><span class="quick-copy"><strong></strong><small></small></span>'
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
    var backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML =
      '<div class="modal camera-detail-modal" role="dialog" aria-modal="true">' +
      '<div class="modal-head"><span class="camera-detail-title"></span><button class="modal-close" aria-label="Đóng">×</button></div>' +
      (isOnline
        ? '<div class="camera-detail-scene"><span class="scene-building"></span><span class="scene-ground"></span><span class="person-shape"></span><span class="bbox"></span><span class="bbox-label">LIVE STREAM</span></div>'
        : '<div class="camera-detail-scene scene-offline"><span class="offline-copy"><span class="dot offline"></span>Tạm thời chưa hoạt động (Camera đang Offline)</span></div>') +
      '<div class="settings-list">' +
      '<div><span>Tên Camera</span><strong data-cam-name></strong></div>' +
      '<div><span>Khu vực lắp đặt</span><strong data-cam-loc></strong></div>' +
      '<div><span>Địa chỉ IP</span><strong data-cam-ip></strong></div>' +
      '<div><span>Luồng RTSP (đã bảo mật)</span><strong data-cam-rtsp></strong></div>' +
      '<div><span>Trạng thái thực tế</span><span class="status ' + (isOnline ? 'online' : 'unknown') + '"><span class="dot ' + (isOnline ? '' : 'offline') + '"></span>' + (isOnline ? 'Đang hoạt động' : 'Tạm thời chưa hoạt động') + '</span></div>' +
      '</div></div>';
    backdrop.querySelector('.camera-detail-title').textContent = cam.name;
    backdrop.querySelector('[data-cam-name]').textContent = cam.name;
    backdrop.querySelector('[data-cam-loc]').textContent = cam.location || 'Chưa đặt vị trí';
    backdrop.querySelector('[data-cam-ip]').textContent = cam.ip_address || '—';
    backdrop.querySelector('[data-cam-rtsp]').textContent = cam.rtsp_url || '—';
    document.getElementById('ss-modal-root').appendChild(backdrop);
    function close() { backdrop.remove(); }
    backdrop.querySelector('.modal-close').addEventListener('click', close);
    backdrop.addEventListener('click', function (e) { if (e.target === backdrop) close(); });
  }

  function renderCamerasViews() {
    var items = state.cameras;
    var regionSelect = document.querySelector('[data-camera-region]');
    if (regionSelect) {
      var prevVal = regionSelect.value || 'all';
      var regions = [];
      items.forEach(function (c) {
        if (c.location && regions.indexOf(c.location) < 0) regions.push(c.location);
      });
      regionSelect.innerHTML = '<option value="all">Tất cả khu vực</option>' +
        regions.map(function (r) { return '<option value="' + r.toLowerCase() + '">' + r + '</option>'; }).join('');
      if (prevVal !== 'all' && regions.map(function (r) { return r.toLowerCase(); }).indexOf(prevVal) >= 0) {
        regionSelect.value = prevVal;
      }
    }

    var searchInput = document.querySelector('[data-camera-search]');
    var statusSelect = document.querySelector('[data-camera-status]');
    var q = searchInput ? searchInput.value.trim().toLowerCase() : '';
    var reg = regionSelect ? regionSelect.value : 'all';
    var st = statusSelect ? statusSelect.value : 'all';

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
      var filteredMonitor = items.filter(function (cam) {
        var matchQ = !q || (cam.name + ' ' + (cam.location || '') + ' ' + (cam.ip_address || '')).toLowerCase().indexOf(q) >= 0;
        var matchReg = reg === 'all' || (cam.location || '').toLowerCase() === reg;
        var matchSt = st === 'all' || (st === 'online' ? cam.status === 'ONLINE' : cam.status !== 'ONLINE');
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
          var card = document.createElement('article');
          card.className = 'camera-card';
          card.innerHTML =
            (isOnline
              ? '<div class="camera-scene"><span class="scene-building"></span><span class="scene-ground"></span><span class="person-shape"></span><span class="camera-live">● LIVE</span></div>'
              : '<div class="camera-scene scene-offline"><span class="camera-live offline-tag"><span class="dot offline"></span>OFFLINE</span><span class="offline-copy">Tạm thời chưa hoạt động</span></div>') +
            '<div class="camera-info"><div><strong></strong><small></small></div><span class="status ' + (isOnline ? 'online' : 'unknown') + '"><span class="dot ' + (isOnline ? '' : 'offline') + '"></span>' + (isOnline ? 'Đang hoạt động' : 'Tạm thời chưa hoạt động') + '</span></div>' +
            '<button class="btn camera-detail">Xem chi tiết</button>';
          card.querySelector('strong').textContent = cam.name;
          card.querySelector('small').textContent = (cam.location || 'Chưa đặt vị trí') + (cam.ip_address ? ' · ' + cam.ip_address : '');
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
    var mst = mgmtStatus ? mgmtStatus.value : 'all';

    var filteredMgmt = items.filter(function (cam) {
      var matchQ = !mq || (cam.name + ' ' + (cam.location || '') + ' ' + (cam.ip_address || '') + ' ' + (cam.rtsp_url || '')).toLowerCase().indexOf(mq) >= 0;
      var matchSt = mst === 'all' || cam.status === mst;
      return matchQ && matchSt;
    });

    var camBody = document.querySelector('[data-page="cameras"] .table tbody');
    var camFoot = document.querySelector('[data-page="cameras"] .table-foot span');
    if (camFoot) camFoot.textContent = 'Tổng cộng ' + filteredMgmt.length + ' camera';
    if (camBody) {
      camBody.replaceChildren();
      if (!filteredMgmt.length) {
        camBody.appendChild(emptyRow(6, items.length ? 'Không có camera khớp bộ lọc.' : 'Chưa có camera nào trong cơ sở dữ liệu.'));
      } else {
        filteredMgmt.forEach(function (cam) {
          var tr = document.createElement('tr');
          var isOnline = cam.status === 'ONLINE';
          tr.innerHTML =
            '<td><strong></strong><small style="display:block;color:var(--ss-muted);font-family:var(--ss-font-mono);font-size:11px;margin-top:2px"></small></td>' +
            '<td></td><td style="font-family:var(--ss-font-mono);font-size:12px"></td>' +
            '<td><span class="status ' + (isOnline ? 'online' : 'offline') + '"><span class="dot ' + (isOnline ? '' : 'red') + '"></span>' + cam.status + '</span></td>' +
            '<td style="font-family:var(--ss-font-mono);font-size:11.5px"></td><td><div class="table-actions"></div></td>';
          var tds = tr.querySelectorAll('td');
          var actionsWrap = tds[5].querySelector('.table-actions');
          tds[0].querySelector('strong').textContent = cam.name;
          tds[0].querySelector('small').textContent = cam.ip_address || 'IP chưa đặt';
          tds[1].textContent = cam.location || '—';
          tds[2].textContent = cam.rtsp_url || '—';
          tds[4].textContent = cam.last_seen ? new Date(cam.last_seen).toLocaleString('vi-VN') : 'Chưa kết nối';

          if (isAdmin()) {
            var editBtn = document.createElement('button');
            editBtn.className = 'btn subtle';
            editBtn.setAttribute('data-edit-cam', cam.id);
            editBtn.textContent = 'Sửa';
            editBtn.addEventListener('click', function () { openCameraFormModal(cam); });

            var delBtn = document.createElement('button');
            delBtn.className = 'btn danger';
            delBtn.setAttribute('data-del-cam', cam.id);
            delBtn.textContent = 'Xóa';
            delBtn.addEventListener('click', function () {
              confirmAction('Bạn có chắc muốn xóa camera "' + cam.name + '"?', function () {
                request('/cameras/' + cam.id, { method: 'DELETE' }).then(function () {
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

  /* ==================== 3. AI MONITORING ==================== */
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
    var personCount = detItems.filter(function (d) { return d.object_type === 'person'; }).length;
    var vehicleCount = detItems.filter(function (d) { return ['car', 'truck', 'vehicle', 'motorcycle', 'bus'].indexOf(d.object_type) >= 0; }).length;
    var otherCount = Math.max(0, detItems.length - personCount - vehicleCount);

    var aiCountAll = document.querySelector('[data-ai-count="all"]');
    var aiCountPerson = document.querySelector('[data-ai-count="person"]');
    var aiCountVehicle = document.querySelector('[data-ai-count="vehicle"]');
    var aiCountOther = document.querySelector('[data-ai-count="other"]');
    if (aiCountAll) aiCountAll.textContent = detItems.length;
    if (aiCountPerson) aiCountPerson.textContent = personCount;
    if (aiCountVehicle) aiCountVehicle.textContent = vehicleCount;
    if (aiCountOther) aiCountOther.textContent = otherCount;

    var evFilter = document.getElementById('ss-ai-event-filter');
    var searchIn = document.getElementById('ss-ai-search');
    var evVal = evFilter ? evFilter.value : 'all';
    var q = searchIn ? searchIn.value.trim().toLowerCase() : '';

    var filtered = detItems.filter(function (d) {
      var isVeh = ['car', 'truck', 'vehicle', 'motorcycle', 'bus'].indexOf(d.object_type) >= 0;
      var matchChip =
        state.aiChipFilter === 'all' ||
        (state.aiChipFilter === 'person' && d.object_type === 'person') ||
        (state.aiChipFilter === 'vehicle' && isVeh) ||
        (state.aiChipFilter === 'other' && d.object_type !== 'person' && !isVeh);
      var matchEv = evVal === 'all' || d.event_type === evVal || (evVal === 'fire' && (d.event_type === 'fire' || d.event_type === 'smoke'));
      var matchQ = !q || (d.object_type + ' ' + d.event_type + ' ' + (d.camera_name || '')).toLowerCase().indexOf(q) >= 0;
      return matchChip && matchEv && matchQ;
    });

    var aiGrid = document.querySelector('[data-page="ai"] .ai-grid');
    var aiFoot = document.querySelector('[data-page="ai"] .table-foot span');
    if (aiFoot) aiFoot.textContent = 'Tổng cộng ' + filtered.length + ' sự kiện';
    if (aiGrid) {
      aiGrid.replaceChildren();
      if (!filtered.length) {
        var empty = document.createElement('div');
        empty.style.color = '#64748b';
        empty.style.padding = '24px';
        empty.innerHTML = '<span class="dot offline"></span>' + (detItems.length ? 'Không có sự kiện AI khớp bộ lọc.' : 'Tạm thời chưa hoạt động — Chưa kết nối luồng Camera RTSP với AI Worker (YOLOv8).');
        aiGrid.appendChild(empty);
      } else {
        filtered.forEach(function (d) {
          var pct = (d.confidence * 100).toFixed(1);
          var isCrit = ['intrusion', 'weapon', 'fire'].indexOf(String(d.event_type).toLowerCase()) >= 0;
          var badgeCls = isCrit ? 'critical' : (d.event_type === 'person' ? 'warning' : 'info');
          var card = document.createElement('article');
          card.className = 'ai-card';
          card.innerHTML =
            '<div class="ai-image"><i class="person-shape"></i><i class="bbox"></i><span class="bbox-label"></span></div>' +
            '<div class="ai-meta"><div class="ai-title"><strong></strong><span class="status ' + badgeCls + '">' + d.event_type.toUpperCase() + '</span></div>' +
            '<div class="ai-sub"></div>' +
            '<div class="confidence"><i style="width:' + pct + '%"></i></div>' +
            '<div class="ai-foot"><span>Độ tin cậy</span><strong>' + pct + '%</strong></div></div>';
          card.querySelector('.bbox-label').textContent = d.object_type + ' ' + d.confidence.toFixed(2);
          card.querySelector('.ai-title strong').textContent = d.object_type + ' · ' + d.event_type;
          card.querySelector('.ai-sub').textContent = (d.camera_name || ('CAM #' + d.camera_id)) + ' · ' + new Date(d.timestamp).toLocaleTimeString('vi-VN');
          aiGrid.appendChild(card);
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
      if (pirDesc) pirDesc.textContent = 'Chưa có tín hiệu từ cảm biến PIR';
      if (pirStatus) { pirStatus.className = 'status unknown'; pirStatus.innerHTML = '<span class="dot offline"></span>Tạm thời chưa hoạt động'; }
      if (mq2Val) { mq2Val.innerHTML = '(— <small>ppm</small>)'; mq2Val.style.color = '#64748b'; }
      if (mq2Bar && mq2Bar.parentElement) { mq2Bar.parentElement.style.display = 'none'; }
      if (mq2Status) { mq2Status.className = 'status unknown'; mq2Status.innerHTML = '<span class="dot offline"></span>Tạm thời chưa hoạt động'; }
      if (lockVal) { lockVal.textContent = '(—)'; lockVal.style.color = '#64748b'; }
      if (relayDesc) relayDesc.textContent = 'Chưa kết nối mạch chấp hành';
      if (actStatus) { actStatus.className = 'status unknown'; actStatus.innerHTML = '<span class="dot offline"></span>Tạm thời chưa hoạt động'; }
      if (cmdTarget) { cmdTarget.className = 'status unknown'; cmdTarget.innerHTML = '<span class="dot offline"></span>Tạm thời chưa hoạt động'; }
      if (toggleRelay) { toggleRelay.classList.remove('on'); toggleRelay.disabled = true; toggleRelay.style.opacity = '0.45'; }
      if (toggleBuzzer) { toggleBuzzer.classList.remove('on'); toggleBuzzer.disabled = true; toggleBuzzer.style.opacity = '0.45'; }
      if (btnLock) { btnLock.disabled = true; btnLock.style.opacity = '0.45'; }
      if (relayText) relayText.textContent = 'Tạm thời chưa hoạt động (Chờ thiết bị kết nối)';
      if (buzzerText) buzzerText.textContent = 'Tạm thời chưa hoạt động (Chờ thiết bị kết nối)';
      if (lockText) lockText.textContent = 'Tạm thời chưa hoạt động (Chờ thiết bị kết nối)';
    } else {
      if (note) note.textContent = activeDev.device_name + ' (' + activeDev.device_code + ') · Đang kết nối trực tuyến';
      if (badge) { badge.className = 'status online'; badge.innerHTML = '<span class="dot"></span>Đang hoạt động'; }
      if (cmdTarget) { cmdTarget.className = 'status online'; cmdTarget.innerHTML = '<span class="dot"></span>' + activeDev.device_code + ' · Đang hoạt động'; }

      var pirDetected = Boolean(activeDev.latest_pir);
      if (pirVal) {
        pirVal.textContent = pirDetected ? 'Phát hiện chuyển động!' : 'Bình thường';
        pirVal.style.color = pirDetected ? '#dc2626' : '#10b981';
      }
      if (pirDesc) pirDesc.textContent = pirDetected ? 'Có người/vật thể trong vùng quét PIR' : 'Không phát hiện chuyển động bất thường';
      if (pirStatus) {
        pirStatus.className = 'status ' + (pirDetected ? 'critical' : 'online');
        pirStatus.innerHTML = pirDetected ? '<span class="dot red"></span>Cảnh báo' : '<span class="dot"></span>Đang hoạt động';
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
        lockVal.textContent = locked ? 'Đang khóa' : 'Đã mở';
        lockVal.style.color = locked ? '#10b981' : '#f59e0b';
      }
      if (relayDesc) relayDesc.textContent = 'Relay: ' + (relayOn ? 'Bật' : 'Tắt') + ' · Còi: ' + (buzzerOn ? 'Bật' : 'Tắt');
      if (actStatus) { actStatus.className = 'status online'; actStatus.innerHTML = '<span class="dot"></span>Đang hoạt động'; }

      if (toggleRelay) { toggleRelay.classList.toggle('on', relayOn); toggleRelay.disabled = false; toggleRelay.style.opacity = '1'; }
      if (toggleBuzzer) { toggleBuzzer.classList.toggle('on', buzzerOn); toggleBuzzer.disabled = false; toggleBuzzer.style.opacity = '1'; }
      if (btnLock) { btnLock.disabled = false; btnLock.style.opacity = '1'; btnLock.textContent = locked ? 'Mở khóa' : 'Khóa cửa'; }
      if (relayText) relayText.textContent = 'Trạng thái thực tế: ' + (relayOn ? 'Đang BẬT' : 'Đang TẮT');
      if (buzzerText) buzzerText.textContent = 'Trạng thái thực tế: ' + (buzzerOn ? 'Đang BẬT' : 'Đang TẮT');
      if (lockText) lockText.textContent = 'Trạng thái thực tế: ' + (locked ? 'Đang KHÓA' : 'Đang MỞ');
    }

    var iotBody = document.querySelector('[data-page="iot"] .table tbody');
    if (iotBody) {
      iotBody.replaceChildren();
      if (!devItems.length) {
        iotBody.appendChild(emptyRow(7, 'Chưa có thiết bị IoT nào được đăng ký.'));
      } else {
        devItems.forEach(function (d) {
          var tr = document.createElement('tr');
          var isOnline = d.status === 'ONLINE';
          tr.innerHTML =
            '<td><strong style="font-family:var(--ss-font-mono)"></strong></td><td></td><td></td>' +
            '<td><span class="status ' + (isOnline ? 'online' : 'offline') + '"><span class="dot ' + (isOnline ? '' : 'red') + '"></span>' + d.status + '</span></td>' +
            '<td style="font-family:var(--ss-font-mono);font-size:12px"></td><td style="font-family:var(--ss-font-mono);font-size:11.5px"></td><td><div class="table-actions"></div></td>';
          var tds = tr.querySelectorAll('td');
          var actionsWrap = tds[6].querySelector('.table-actions');
          tds[0].querySelector('strong').textContent = d.device_code;
          tds[1].textContent = d.device_name;
          tds[2].textContent = d.device_type;
          tds[4].textContent = 'PIR: ' + (d.latest_pir ? 'Có' : 'Không') + ' · MQ-2: ' + (d.latest_mq2 != null ? d.latest_mq2 + ' ppm' : '—');
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
    var backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML =
      '<div class="modal" role="dialog" aria-modal="true">' +
      '<div class="modal-head">' + (isEdit ? 'Cập nhật Camera' : 'Thông tin Camera mới') + '<button class="modal-close" aria-label="Đóng">×</button></div>' +
      '<div class="modal-grid">' +
      '<div class="modal-field"><label for="ss-field-0">Tên Camera *</label><input id="ss-field-0" type="text" placeholder="Nhập tên camera (vd: CAM-01 · Cổng chính)"></div>' +
      '<div class="modal-field"><label for="ss-field-1">Địa chỉ IP</label><input id="ss-field-1" type="text" placeholder="Nhập địa chỉ IP (vd: 192.168.1.21)"></div>' +
      '<div class="modal-field"><label for="ss-field-2">RTSP URL *</label><input id="ss-field-2" type="text" placeholder="rtsp://192.168.1.21:554/stream1"></div>' +
      '<div class="modal-field"><label for="ss-field-3">Khu vực lắp đặt</label><input id="ss-field-3" type="text" placeholder="Nhập khu vực lắp đặt (vd: Cổng chính)"></div>' +
      '<div class="modal-field"><label for="ss-field-4">Trạng thái</label><select id="ss-field-4" class="select" style="width:100%"><option value="OFFLINE">OFFLINE (Tạm thời chưa hoạt động)</option><option value="ONLINE">ONLINE (Trực tuyến)</option></select></div>' +
      '</div>' +
      '<div class="modal-foot"><button class="btn" data-cancel>Hủy</button><button class="btn primary" data-save>Lưu thông tin</button></div>' +
      '</div>';
    if (isEdit) {
      backdrop.querySelector('#ss-field-0').value = existingCam.name || '';
      backdrop.querySelector('#ss-field-1').value = existingCam.ip_address || '';
      backdrop.querySelector('#ss-field-2').value = existingCam.rtsp_url || '';
      backdrop.querySelector('#ss-field-3').value = existingCam.location || '';
      backdrop.querySelector('#ss-field-4').value = existingCam.status === 'OFFLINE' ? 'OFFLINE' : 'ONLINE';
    }
    document.getElementById('ss-modal-root').appendChild(backdrop);
    function close() { backdrop.remove(); }
    backdrop.querySelector('.modal-close').addEventListener('click', close);
    backdrop.querySelector('[data-cancel]').addEventListener('click', close);
    backdrop.addEventListener('click', function (e) { if (e.target === backdrop) close(); });
    backdrop.querySelector('[data-save]').addEventListener('click', function () {
      var payload = {
        name: backdrop.querySelector('#ss-field-0').value.trim(),
        ip_address: backdrop.querySelector('#ss-field-1').value.trim() || null,
        rtsp_url: backdrop.querySelector('#ss-field-2').value.trim(),
        location: backdrop.querySelector('#ss-field-3').value.trim() || null,
        status: backdrop.querySelector('#ss-field-4').value
      };
      if (!payload.name || (!isEdit && !payload.rtsp_url)) {
        toast('Vui lòng nhập Tên Camera và RTSP URL.');
        return;
      }
      var method = isEdit ? 'PUT' : 'POST';
      var path = isEdit ? ('/cameras/' + existingCam.id) : '/cameras';
      request(path, { method: method, body: JSON.stringify(payload) }).then(function () {
        close();
        toast(isEdit ? 'Đã cập nhật thông tin camera.' : 'Đã thêm camera mới vào cơ sở dữ liệu.');
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
    var camOpts = state.cameras.map(function (c) { return '<option value="' + c.id + '">' + c.name + '</option>'; }).join('');
    backdrop.innerHTML =
      '<div class="modal" role="dialog" aria-modal="true">' +
      '<div class="modal-head">Gửi sự kiện nhận diện AI<button class="modal-close" aria-label="Đóng">×</button></div>' +
      '<div class="modal-grid">' +
      '<div class="modal-field"><label for="ss-sim-cam">Chọn Camera</label><select id="ss-sim-cam" class="select" style="width:100%">' + camOpts + '</select></div>' +
      '<div class="modal-field"><label for="ss-sim-obj">Đối tượng nhận diện (Object)</label><select id="ss-sim-obj" class="select" style="width:100%"><option value="person">person (Người)</option><option value="car">car (Xe ô tô)</option><option value="truck">truck (Xe tải)</option><option value="fire">fire (Lửa / Cháy)</option><option value="weapon">weapon (Vũ khí)</option></select></div>' +
      '<div class="modal-field"><label for="ss-sim-evt">Loại sự kiện (Event Type)</label><select id="ss-sim-evt" class="select" style="width:100%"><option value="intrusion">intrusion (Xâm nhập -> Cảnh báo CRITICAL)</option><option value="person">person (Phát hiện người -> Cảnh báo WARNING)</option><option value="fire">fire (Cháy nổ -> Cảnh báo CRITICAL)</option><option value="object">object (Thông thường)</option></select></div>' +
      '<div class="modal-field"><label for="ss-sim-conf">Độ tin cậy (0.10 – 0.99)</label><input id="ss-sim-conf" type="number" step="0.01" min="0.1" max="0.99" value="0.95"></div>' +
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
      request('/detections', {
        method: 'POST',
        body: JSON.stringify({ camera_id: camId, object_type: obj, event_type: evt, confidence: conf, image_path: 'uploads/events/sim.jpg' })
      }).then(function (res) {
        close();
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
      '<div class="modal-field"><label for="ss-sen-pir">Cảm biến chuyển động PIR</label><select id="ss-sen-pir" class="select" style="width:100%"><option value="false">Không có chuyển động (Bình thường)</option><option value="true">Phát hiện chuyển động (PIR = TRUE)</option></select></div>' +
      '<div class="modal-field"><label for="ss-sen-mq2">Nồng độ khí gas MQ-2 (ppm)</label><input id="ss-sen-mq2" type="number" min="0" max="10000" value="' + (active.latest_mq2 != null ? active.latest_mq2 : 350) + '"></div>' +
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

    // Monitor & Cameras filters
    ['[data-camera-search]', '[data-camera-region]', '[data-camera-status]', '#ss-cam-mgmt-search', '#ss-cam-mgmt-status'].forEach(function (sel) {
      var el = document.querySelector(sel);
      if (el) {
        el.addEventListener('input', renderCamerasViews);
        el.addEventListener('change', renderCamerasViews);
      }
    });

    // AI filters
    document.querySelectorAll('#ss-ai-filters .filter-chip').forEach(function (chip) {
      chip.addEventListener('click', function () {
        document.querySelectorAll('#ss-ai-filters .filter-chip').forEach(function (c) { c.classList.remove('active'); });
        chip.classList.add('active');
        state.aiChipFilter = chip.dataset.aiFilter || 'all';
        renderAiView();
      });
    });
    ['#ss-ai-event-filter', '#ss-ai-search'].forEach(function (sel) {
      var el = document.querySelector(sel);
      if (el) {
        el.addEventListener('input', renderAiView);
        el.addEventListener('change', renderAiView);
      }
    });

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
        sendDeviceCommand(nextAction, nextAction === 'relay_on' ? 'BẬT Relay đèn khu vực' : 'TẮT Relay đèn khu vực');
      });
    }

    var toggleBuzzer = document.getElementById('ss-toggle-buzzer');
    if (toggleBuzzer) {
      toggleBuzzer.addEventListener('click', function () {
        var active = getActiveDevice();
        var nextAction = (active && active.buzzer) ? 'buzzer_off' : 'buzzer_on';
        sendDeviceCommand(nextAction, nextAction === 'buzzer_on' ? 'BẬT Còi báo động' : 'TẮT Còi báo động');
      });
    }

    var btnLock = document.getElementById('ss-btn-lock-toggle');
    if (btnLock) {
      btnLock.addEventListener('click', function () {
        var active = getActiveDevice();
        var isLocked = !active || active.lock_engaged !== false;
        var nextAction = isLocked ? 'unlock' : 'lock';
        sendDeviceCommand(nextAction, isLocked ? 'MỞ KHÓA cửa chính' : 'KHÓA cửa chính');
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
    var socketUrl = (location.protocol === 'https:' ? 'wss://' : 'ws://') + location.host + '/ws/dashboard?token=' + encodeURIComponent(token);
    activeSocket = new WebSocket(socketUrl);
    activeSocket.onmessage = function () { refreshDataOnly(); };
    activeSocket.onclose = function () { activeSocket = null; };
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
      request('/users'),
      request('/system/health'),
      request('/system/settings')
    ]).then(function (results) {
      applyUserRole(results[0]);
      state.overview = results[1] || {};
      state.alerts = (results[2] && results[2].items) || [];
      state.detStats = (results[3] && results[3].items) || [];
      state.cameras = (results[4] && results[4].items) || [];
      state.devices = (results[5] && results[5].items) || [];
      state.logs = (results[6] && results[6].items) || [];
      state.detections = (results[7] && results[7].items) || [];
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
