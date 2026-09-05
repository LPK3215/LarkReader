/* ============================================================================
   LarkReader Landing · script.js
   Generator: 2026-09-06 · WorkBuddy
   职责：主题切换 + 滚动进度条 + 数字动画 + 返回顶部 + 平滑滚动
   ============================================================================ */
(function () {
  'use strict';

  /* ---------- 1. 主题切换（auto / light / dark 三态循环）---------- */
  var STORAGE_KEY = 'larkreader.theme';
  var themeIcon = document.querySelector('.theme-icon');

  function getStoredTheme() {
    try { return localStorage.getItem(STORAGE_KEY); } catch (e) { return null; }
  }
  function setStoredTheme(mode) {
    try { localStorage.setItem(STORAGE_KEY, mode); } catch (e) { /* ignore */ }
  }
  function effectiveTheme(mode) {
    if (mode === 'auto') {
      return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    }
    return mode;
  }
  function applyTheme(mode) {
    var eff = effectiveTheme(mode);
    if (mode === 'auto') {
      document.body.removeAttribute('data-theme');
    } else {
      document.body.setAttribute('data-theme', mode);
    }
    if (themeIcon) themeIcon.setAttribute('data-mode', mode);
  }

  applyTheme(getStoredTheme() || 'auto');

  var toggle = document.getElementById('theme-toggle');
  if (toggle) {
    toggle.addEventListener('click', function () {
      var cur = getStoredTheme() || 'auto';
      var next = cur === 'auto' ? 'light' : (cur === 'light' ? 'dark' : 'auto');
      setStoredTheme(next);
      applyTheme(next);
    });
  }

  // 跟随系统偏好变化
  if (window.matchMedia) {
    var mql = window.matchMedia('(prefers-color-scheme: dark)');
    if (mql.addEventListener) {
      mql.addEventListener('change', function () {
        var cur = getStoredTheme() || 'auto';
        if (cur === 'auto') applyTheme('auto');
      });
    }
  }

  /* ---------- 2. 滚动进度条 ---------- */
  var progress = document.getElementById('scroll-progress');
  var topbar = document.getElementById('topbar');
  function updateScrollProgress() {
    var h = document.documentElement;
    var b = document.body;
    var scrollTop = h.scrollTop || b.scrollTop || 0;
    var scrollHeight = (h.scrollHeight || b.scrollHeight || 0) - h.clientHeight;
    var pct = scrollHeight > 0 ? (scrollTop / scrollHeight) * 100 : 0;
    if (progress) progress.style.width = Math.min(100, Math.max(0, pct)) + '%';

    // 顶栏阴影（滚动超过一屏）
    if (topbar) {
      if (scrollTop > 8) topbar.classList.add('scrolled');
      else topbar.classList.remove('scrolled');
    }
  }
  window.addEventListener('scroll', updateScrollProgress, { passive: true });
  updateScrollProgress();

  /* ---------- 3. 返回顶部按钮 ---------- */
  var backTop = document.getElementById('back-top');
  function updateBackTop() {
    if (!backTop) return;
    var scrollTop = window.pageYOffset || document.documentElement.scrollTop;
    if (scrollTop > window.innerHeight * 0.6) backTop.classList.add('show');
    else backTop.classList.remove('show');
  }
  if (backTop) {
    backTop.addEventListener('click', function () {
      window.scrollTo({ top: 0, behavior: 'smooth' });
    });
  }
  window.addEventListener('scroll', updateBackTop, { passive: true });
  updateBackTop();

  /* ---------- 4. Hero 数字计数动画 ---------- */
  var nums = document.querySelectorAll('.stat-num[data-target]');
  function animateNumber(el) {
    var target = parseInt(el.getAttribute('data-target'), 10) || 0;
    var duration = 1200;
    var startTime = null;
    function tick(t) {
      if (startTime === null) startTime = t;
      var elapsed = t - startTime;
      var pct = Math.min(1, elapsed / duration);
      // ease-out cubic
      var eased = 1 - Math.pow(1 - pct, 3);
      var val = Math.round(target * eased);
      el.textContent = val;
      if (pct < 1) requestAnimationFrame(tick);
      else el.textContent = target;
    }
    requestAnimationFrame(tick);
  }
  if ('IntersectionObserver' in window && nums.length) {
    var seen = new WeakSet();
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting && !seen.has(entry.target)) {
          seen.add(entry.target);
          animateNumber(entry.target);
          io.unobserve(entry.target);
        }
      });
    }, { threshold: 0.4 });
    nums.forEach(function (n) { io.observe(n); });
  } else {
    // 降级：直接显示目标值
    nums.forEach(function (n) {
      n.textContent = n.getAttribute('data-target') || '0';
    });
  }

  /* ---------- 5. 锚点平滑滚动（兼容老浏览器）---------- */
  document.querySelectorAll('a[href^="#"]').forEach(function (a) {
    a.addEventListener('click', function (e) {
      var href = a.getAttribute('href');
      if (!href || href === '#') return;
      var target = document.querySelector(href);
      if (!target) return;
      e.preventDefault();
      var topbarHeight = topbar ? topbar.offsetHeight : 0;
      var rect = target.getBoundingClientRect();
      var top = rect.top + window.pageYOffset - topbarHeight - 8;
      window.scrollTo({ top: top, behavior: 'smooth' });
      // 更新 hash 不触发额外滚动
      if (history.replaceState) history.replaceState(null, '', href);
    });
  });

  /* ---------- 6. 卡片入场动画（可选，轻量）---------- */
  if ('IntersectionObserver' in window) {
    var revealIo = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) {
          entry.target.classList.add('revealed');
          revealIo.unobserve(entry.target);
        }
      });
    }, { threshold: 0.12, rootMargin: '0px 0px -40px 0px' });

    document.querySelectorAll('.scene-card, .feature, .step, .dl-card, .faq-item').forEach(function (el) {
      el.classList.add('reveal-init');
      revealIo.observe(el);
    });
  }

  /* ---------- 7. GitHub URL 占位提示（仅占位，方便小马后续替换）---------- */
  // 当前所有 GitHub 链接 href="https://github.com/" —— 小马替换为真实仓库地址即可
  // 后续可以批量用 selector 替换：document.querySelectorAll('a[href="https://github.com/"]').forEach(...)
})();