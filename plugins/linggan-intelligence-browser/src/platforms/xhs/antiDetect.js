// ========== 1. 有界页面加载等待 ==========

export function waitForPageSettle(delayMs = 120) {
  return new Promise((resolve) => setTimeout(resolve, Math.max(0, Math.round(Number(delayMs) || 0))));
}

// ========== 2. 有界页面加载滚动 ==========

/**
 * Legacy function name retained for existing callers. It performs a finite, observable page
 * loading scroll; it does not attempt to imitate or evade a platform's detection systems.
 */
export async function humanScroll(element, distance, settleDelayMs = 180) {
  if (!element || typeof element.scrollBy !== 'function') return;
  const totalDistance = Math.abs(Number(distance || 0));
  if (totalDistance <= 0) return;

  element.scrollBy({ top: distance, behavior: 'auto' });
  await waitForPageSettle(settleDelayMs);
}

// ========== 3. 分级节流 ==========

/**
 * 根据已采集数量动态调整等待时间
 * 采集越多，等待越长，为页面加载和本地保存留出稳定窗口
 */
export function throttle(collectedCount) {
  if (collectedCount < 10) {
    return waitForPageSettle(450);
  } else if (collectedCount < 30) {
    return waitForPageSettle(650);
  } else {
    return waitForPageSettle(850);
  }
}

// ========== 4. 滑块验证码检测 ==========

export function detectCaptcha() {
  const captchaSelectors = [
    '.captcha-container',
    '.verify-container',
    '#captcha',
    '.slide-verify',
    'iframe[src*="captcha"]',
    'iframe[src*="verify"]',
  ];

  for (const selector of captchaSelectors) {
    const el = document.querySelector(selector);
    if (el && el.offsetHeight > 0) {
      return true;
    }
  }
  return false;
}

/**
 * 启动验证码监控（轮询）
 */
export function watchCaptchaByPolling(onDetected, intervalMs = 1000) {
  const timer = setInterval(() => {
    if (detectCaptcha()) {
      clearInterval(timer);
      onDetected();
    }
  }, intervalMs);

  return {
    disconnect: () => clearInterval(timer),
  };
}

/**
 * 启动验证码监控（保留原函数签名，内部使用轮询）
 */
export function watchCaptcha(onDetected) {
  return watchCaptchaByPolling(onDetected, 1000);
}

// ========== 5. 验证码暂停恢复 UI ==========

/**
 * 注入验证码暂停浮窗
 * @param {Function} onResume - 用户点击继续后的回调，参数 'resume' | 'stop'
 * @returns {Function} dismiss - 手动移除浮窗
 */
export function showCaptchaPauseOverlay(options = {}) {
  const timeoutMs = Math.max(0, Number(options?.timeoutMs || 0) || 0);
  document.querySelector('.lgbbb-captcha-overlay')?.remove();

  const overlay = document.createElement('div');
  overlay.className = 'lgbbb-captcha-overlay';
  overlay.innerHTML = `
    <div class="lgbbb-captcha-dialog">
      <div class="lgbbb-captcha-icon">!</div>
      <div class="lgbbb-captcha-title">采集已暂停</div>
      <div class="lgbbb-captcha-desc">
        检测到滑块验证码，请先完成验证。<br/>
        验证通过后，点击下方按钮继续采集。
      </div>
      <button class="lgbbb-captcha-resume-btn">继续采集</button>
      <button class="lgbbb-captcha-stop-btn">停止采集</button>
    </div>
  `;

  Object.assign(overlay.style, {
    position: 'fixed',
    top: '0', left: '0', right: '0', bottom: '0',
    background: 'rgba(0,0,0,0.5)',
    zIndex: '2147483647',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
  });

  const dialog = overlay.querySelector('.lgbbb-captcha-dialog');
  Object.assign(dialog.style, {
    background: '#fff',
    borderRadius: '16px',
    padding: '32px 40px',
    textAlign: 'center',
    boxShadow: '0 8px 32px rgba(0,0,0,0.2)',
    maxWidth: '360px',
  });

  overlay.querySelector('.lgbbb-captcha-icon').style.fontSize = '48px';
  overlay.querySelector('.lgbbb-captcha-title').style.cssText =
    'font-size:20px;font-weight:700;margin:12px 0 8px;color:#333;';
  overlay.querySelector('.lgbbb-captcha-desc').style.cssText =
    'font-size:14px;color:#666;line-height:1.6;margin-bottom:24px;';

  const resumeBtn = overlay.querySelector('.lgbbb-captcha-resume-btn');
  resumeBtn.style.cssText =
    'background:#ff4757;color:#fff;border:none;border-radius:8px;padding:12px 32px;' +
    'font-size:16px;font-weight:600;cursor:pointer;margin-right:12px;';

  const stopBtn = overlay.querySelector('.lgbbb-captcha-stop-btn');
  stopBtn.style.cssText =
    'background:#e0e0e0;color:#333;border:none;border-radius:8px;padding:12px 32px;' +
    'font-size:16px;cursor:pointer;';

  const dismiss = () => overlay.remove();

  return new Promise((resolve) => {
    let timer = null;
    const finish = (action) => {
      if (timer) window.clearTimeout(timer);
      dismiss();
      resolve(action);
    };
    resumeBtn.addEventListener('click', () => {
      finish('resume');
    });
    stopBtn.addEventListener('click', () => {
      finish('stop');
    });
    document.body.appendChild(overlay);
    if (timeoutMs > 0) {
      timer = window.setTimeout(() => {
        finish('timeout');
      }, timeoutMs);
    }
  });
}
