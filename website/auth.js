'use strict';
(() => {
  const URL = 'https://ieafdutkxpyqwmfedwqy.supabase.co';
  const KEY = 'sb_publishable_UJ7kEzA4zWNf234jJpbl9A_pF4UlHue';
  const STORAGE = 'sf-web-session';
  let session = null, user = null, admin = null, refreshTimer, refreshPromise;
  let epoch = 0, ready = false, busy = false, mode = 'signin', dialog, form, opener;
  let actionController = null, sessionController = new AbortController();
  let messageKey = '', errorKey = '', serverError = '';
  const nodes = {};
  const messages = {
    signin: ['Sign in', 'Увійти'], signup: ['Create account', 'Створити акаунт'],
    welcome: ['Welcome back', 'З поверненням'], create: ['Create your account', 'Створіть акаунт'],
    intro: ['Sign in with your email and password.', 'Увійдіть за допомогою пошти та пароля.'],
    signupIntro: ['Confirm your email before signing in. Registration does not grant admin access.', 'Підтвердьте пошту перед входом. Реєстрація не надає прав адміністратора.'],
    email: ['Email', 'Електронна пошта'], password: ['Password', 'Пароль'],
    confirmPassword: ['Confirm password', 'Повторіть пароль'], close: ['Close account dialog', 'Закрити вікно акаунта'],
    switchSignup: ['New here? Create an account', 'Вперше тут? Створіть акаунт'],
    switchSignin: ['Already registered? Sign in', 'Уже зареєстровані? Увійдіть'],
    account: ['Your account', 'Ваш акаунт'], accountLabel: ['Sign in / Register', 'Увійти / Реєстрація'],
    logout: ['Sign out', 'Вийти'], working: ['Please wait…', 'Зачекайте…'],
    confirmation: ['Check your inbox and confirm your email, then return here to sign in. If you already have an account, sign in after confirming your email.', 'Перевірте пошту та підтвердьте адресу, потім поверніться сюди для входу. Якщо акаунт уже існує, увійдіть після підтвердження пошти.'],
    confirmedRequired: ['Confirm your email before signing in. Check your inbox for the confirmation link.', 'Підтвердьте пошту перед входом. Перевірте лист із посиланням для підтвердження.'],
    mismatch: ['Passwords do not match.', 'Паролі не збігаються.'],
    failure: ['Unable to complete authentication. Check your details and try again.', 'Не вдалося виконати автентифікацію. Перевірте дані та спробуйте знову.'],
    network: ['Cannot reach the account service. Check your connection and try again.', 'Сервіс акаунтів недоступний. Перевірте з’єднання та спробуйте знову.'],
    expired: ['Your session has expired. Please sign in again.', 'Сеанс завершився. Увійдіть знову.'],
    invalidLink: ['This confirmation link is invalid or has expired. Please sign in or request a new confirmation from the account service.', 'Посилання підтвердження недійсне або застаріло. Увійдіть або запитайте нове підтвердження в сервісі акаунтів.'],
    rateLimit: ['Too many attempts. Please try again later.', 'Забагато спроб. Спробуйте пізніше.'],
    verified: ['Verified admin', 'Адміністратор підтверджений'],
    noAdmin: ['Admin access: no', 'Права адміністратора: ні'],
    unknownAdmin: ['Admin access could not be verified.', 'Не вдалося перевірити права адміністратора.'],
    checking: ['Checking your session…', 'Перевіряємо сеанс…'],
    privacy: ['Your password is never saved. This tab keeps your session until you sign out or close the tab.', 'Пароль ніколи не зберігається. Сеанс зберігається у цій вкладці до виходу або її закриття.'],
    logoutFailed: ['Signed out in this tab. The server could not be reached to revoke the session.', 'Вихід у цій вкладці виконано. Не вдалося зв’язатися із сервером для відкликання сеансу.']
  };
  function t(key) { return messages[key]?.[document.documentElement.lang.split('-')[0] === 'uk' ? 1 : 0] || ''; }
  function storageRemove() { try { sessionStorage.removeItem(STORAGE); } catch {} }
  function store() {
    try { sessionStorage.setItem(STORAGE, JSON.stringify(session)); } catch { /* Memory-only session when storage is unavailable. */ }
  }
  function tokens(data) {
    if (!data || typeof data.access_token !== 'string' || !data.access_token || typeof data.refresh_token !== 'string' || !data.refresh_token) throw new Error('Invalid session');
    const expiry = Number(data.expires_at) || Date.now() / 1000 + Number(data.expires_in || 3600);
    if (!Number.isFinite(expiry)) throw new Error('Invalid expiry');
    return { access_token: data.access_token, refresh_token: data.refresh_token, expires_at: expiry };
  }
  async function request(path, { method = 'GET', token, body, signal } = {}) {
    const headers = { apikey: KEY, Accept: 'application/json' };
    if (token) headers.Authorization = 'Bearer ' + token;
    if (body !== undefined) headers['Content-Type'] = 'application/json';
    let response;
    try {
      response = await fetch(URL + path, { method, headers, ...(body !== undefined ? { body: JSON.stringify(body) } : {}), signal, credentials: 'omit', redirect: 'error', cache: 'no-store', referrerPolicy: 'no-referrer' });
    } catch (error) {
      if (error.name === 'AbortError') throw error;
      throw Object.assign(new Error('Network unavailable'), { key: 'network' });
    }
    let data = null;
    try { data = await response.json(); } catch {}
    if (!response.ok) {
      const key = response.status === 429 ? 'rateLimit' : data?.error_code === 'email_not_confirmed' || data?.code === 'email_not_confirmed' ? 'confirmedRequired' : 'failure';
      throw Object.assign(new Error('Authentication request failed'), { key, status: response.status });
    }
    return data;
  }
  function clearSession() {
    epoch++;
    clearTimeout(refreshTimer);
    sessionController.abort();
    sessionController = new AbortController();
    refreshPromise = null;
    session = null; user = null; admin = null;
    storageRemove();
    paint();
  }
  function armRefresh() {
    clearTimeout(refreshTimer);
    if (session) refreshTimer = setTimeout(() => refresh().catch(() => {}), Math.min(2147483647, Math.max(1000, (session.expires_at * 1000 - Date.now()) - 60000)));
  }
  async function checkAdmin(version) {
    try {
      const result = await request('/rest/v1/rpc/soundflow_is_admin', { method: 'POST', token: session.access_token, body: {}, signal: sessionController.signal });
      if (version === epoch) { admin = typeof result === 'boolean' ? result : null; paint(); }
    } catch { if (version === epoch) { admin = null; paint(); } }
  }
  async function accept(candidate, signal, version = epoch) {
    const verified = await request('/auth/v1/user', { token: candidate.access_token, signal });
    if (version !== epoch || signal?.aborted) return false;
    if (!verified || typeof verified.id !== 'string' || typeof verified.email !== 'string') throw new Error('Invalid user');
    if (!verified.email_confirmed_at && !verified.confirmed_at) throw Object.assign(new Error('Confirmation required'), { key: 'confirmedRequired' });
    session = candidate; user = { id: verified.id, email: verified.email }; admin = null;
    store(); armRefresh(); paint();
    void checkAdmin(version);
    return true;
  }
  async function refresh() {
    if (refreshPromise) return refreshPromise;
    if (!session) return;
    const version = epoch, current = session;
    refreshPromise = (async () => {
      try {
        const data = await request('/auth/v1/token?grant_type=refresh_token', { method: 'POST', body: { refresh_token: current.refresh_token }, signal: sessionController.signal });
        const candidate = tokens(data);
        if (version !== epoch) return;
        // Save rotated tokens immediately, but only expose a user after verification.
        session = candidate; user = null; admin = null; store(); paint();
        await accept(candidate, sessionController.signal, version);
      } catch (error) {
        if (version === epoch && error.name !== 'AbortError') { clearSession(); errorKey = error.key || 'expired'; paint(); }
        throw error;
      } finally { if (version === epoch) refreshPromise = null; }
    })();
    return refreshPromise;
  }
  function element(tag, className, parent) {
    const el = document.createElement(tag);
    if (className) el.className = className;
    if (parent) parent.append(el);
    return el;
  }
  function build() {
    if (dialog) return;
    dialog = element('dialog', 'sf-auth-dialog', document.body);
    dialog.id = 'auth-dialog'; dialog.setAttribute('aria-labelledby', 'sf-auth-title'); dialog.setAttribute('aria-describedby', 'sf-auth-description');
    form = element('form', 'auth-box', dialog);
    nodes.close = element('button', 'auth-close', form); nodes.close.type = 'button'; nodes.close.textContent = '×';
    nodes.close.addEventListener('click', () => dialog.close());
    const kicker = element('p', 'auth-kicker', form); kicker.textContent = 'SOUNDFLOW ACCOUNT';
    nodes.title = element('h2', '', form); nodes.title.id = 'sf-auth-title';
    nodes.description = element('p', 'auth-desc', form); nodes.description.id = 'sf-auth-description';
    nodes.fields = element('div', 'auth-fields', form);
    for (const name of ['email', 'password', 'confirmPassword']) {
      const label = element('label', '', nodes.fields);
      nodes[name + 'Label'] = element('span', '', label);
      const input = element('input', '', label);
      input.name = name === 'confirmPassword' ? 'confirm' : name;
      input.type = name === 'email' ? 'email' : 'password';
      input.autocomplete = name === 'email' ? 'email' : name === 'confirmPassword' ? 'new-password' : 'current-password';
      input.maxLength = name === 'email' ? 254 : 1024;
      if (name !== 'email') input.minLength = 8;
      input.required = name !== 'confirmPassword';
      nodes[name] = input;
      if (name === 'confirmPassword') nodes.confirmWrap = label;
    }
    nodes.email.autocapitalize = 'none'; nodes.email.spellcheck = false;
    nodes.admin = element('p', 'auth-admin', form);
    nodes.error = element('p', 'auth-error', form); nodes.error.setAttribute('role', 'alert');
    nodes.status = element('p', 'auth-confirm', form); nodes.status.setAttribute('role', 'status'); nodes.status.setAttribute('aria-live', 'polite');
    nodes.submit = element('button', 'auth-submit', form); nodes.submit.type = 'submit';
    nodes.mode = element('button', 'auth-mode', form); nodes.mode.type = 'button';
    nodes.logout = element('button', 'auth-submit auth-logout', form); nodes.logout.type = 'button'; nodes.logout.dataset.authLogout = '';
    nodes.privacy = element('p', 'auth-small', form);
    nodes.mode.addEventListener('click', () => {
      if (busy) return;
      mode = mode === 'signin' ? 'signup' : 'signin';
      errorKey = messageKey = serverError = '';
      nodes.password.value = nodes.confirmPassword.value = '';
      paint(); nodes.email.focus();
    });
    form.addEventListener('submit', submit);
    dialog.addEventListener('cancel', () => actionController?.abort());
    dialog.addEventListener('close', () => {
      actionController?.abort(); actionController = null; busy = false;
      nodes.password.value = nodes.confirmPassword.value = '';
      if (opener?.isConnected) opener.focus();
      paint();
    });
    paint();
  }
  function paint() {
    document.querySelectorAll('[data-account-label]').forEach(el => { el.textContent = user?.email || t('accountLabel'); });
    document.querySelectorAll('[data-admin-status]').forEach(el => {
      el.hidden = !user || admin !== true;
      el.textContent = admin === true ? t('verified') : '';
    });
    if (!dialog) return;
    const signedIn = Boolean(user), signup = mode === 'signup';
    nodes.close.setAttribute('aria-label', t('close'));
    nodes.title.textContent = t(signedIn ? 'account' : signup ? 'create' : 'welcome');
    nodes.description.textContent = signedIn ? user.email : t(signup ? 'signupIntro' : 'intro');
    nodes.fields.hidden = signedIn;
    nodes.confirmWrap.hidden = !signup;
    nodes.confirmPassword.required = signup && !signedIn;
    nodes.email.required = nodes.password.required = !signedIn;
    nodes.password.autocomplete = signup ? 'new-password' : 'current-password';
    nodes.password.minLength = signup ? 8 : 1;
    for (const name of ['email', 'password', 'confirmPassword']) {
      nodes[name + 'Label'].textContent = t(name);
      nodes[name].disabled = busy || !ready;
    }
    nodes.admin.hidden = !signedIn;
    nodes.admin.textContent = signedIn ? t(admin === true ? 'verified' : admin === false ? 'noAdmin' : 'unknownAdmin') : '';
    nodes.error.textContent = serverError || t(errorKey);
    nodes.status.textContent = !ready ? t('checking') : t(messageKey);
    nodes.submit.hidden = nodes.mode.hidden = signedIn;
    nodes.submit.textContent = t(busy ? 'working' : signup ? 'signup' : 'signin');
    nodes.submit.disabled = nodes.mode.disabled = busy || !ready;
    nodes.mode.textContent = t(signup ? 'switchSignin' : 'switchSignup');
    nodes.logout.hidden = !signedIn;
    nodes.logout.disabled = busy;
    nodes.logout.textContent = t(busy ? 'working' : 'logout');
    nodes.privacy.textContent = t('privacy');
    form.setAttribute('aria-busy', String(busy || !ready));
  }
  function open(trigger) {
    build();
    if (dialog.open) return;
    opener = trigger instanceof HTMLElement ? trigger : document.activeElement;
    errorKey = messageKey = serverError = '';
    paint(); dialog.showModal();
    (user ? nodes.logout : nodes.email).focus();
  }
  async function submit(event) {
    event.preventDefault();
    if (busy || !ready || user || !form.reportValidity()) return;
    errorKey = messageKey = serverError = '';
    if (mode === 'signup' && nodes.password.value !== nodes.confirmPassword.value) { errorKey = 'mismatch'; paint(); nodes.confirmPassword.focus(); return; }
    busy = true; paint();
    const controller = new AbortController(); actionController = controller;
    const version = epoch;
    const email = nodes.email.value.trim(), password = nodes.password.value;
    try {
      if (mode === 'signup') {
        const redirect = location.origin + location.pathname + location.search;
        await request('/auth/v1/signup?redirect_to=' + encodeURIComponent(redirect), { method: 'POST', body: { email, password }, signal: controller.signal });
        if (controller.signal.aborted) return;
        // Email confirmation remains mandatory even if the service returns a session.
        if (version !== epoch || controller.signal.aborted) return;
        mode = 'signin'; messageKey = 'confirmation';
        nodes.password.value = nodes.confirmPassword.value = '';
      } else {
        const data = await request('/auth/v1/token?grant_type=password', { method: 'POST', body: { email, password }, signal: controller.signal });
        if (await accept(tokens(data), controller.signal, version)) {
          nodes.password.value = nodes.confirmPassword.value = '';
        }
      }
    } catch (error) {
      if (error.name !== 'AbortError' && !controller.signal.aborted) errorKey = error.key || 'failure';
    } finally {
      if (actionController === controller) { busy = false; actionController = null; paint(); }
    }
  }
  async function logout() {
    if (busy) return;
    busy = true;
    const token = session?.access_token;
    actionController?.abort();
    clearSession(); paint();
    const controller = new AbortController(); actionController = controller;
    // Local tokens are removed immediately even if remote revocation fails.
    try { if (token) await request('/auth/v1/logout', { method: 'POST', token, signal: controller.signal }); }
    catch (error) { if (error.name !== 'AbortError') messageKey = 'logoutFailed'; }
    finally { if (actionController === controller) { actionController = null; busy = false; paint(); } }
  }
  async function initialize() {
    let candidate;
    const hash = new URLSearchParams(location.hash.slice(1));
    const hasCallback = hash.has('access_token') || hash.has('refresh_token') || hash.has('error_description');
    if (hasCallback) {
      try { history.replaceState(null, '', location.pathname + location.search); } catch {}
      try {
        if (hash.has('error_description')) throw new Error('Invalid callback');
        candidate = tokens(Object.fromEntries(hash));
      } catch { errorKey = 'invalidLink'; }
    } else {
      try { const raw = JSON.parse(sessionStorage.getItem(STORAGE) || 'null'); if (raw) candidate = tokens(raw); } catch { storageRemove(); }
    }
    if (candidate) {
      try {
        session = candidate;
        if (candidate.expires_at * 1000 <= Date.now() + 60000) await refresh();
        else await accept(candidate, sessionController.signal);
      } catch (error) { clearSession(); errorKey = error.key || 'expired'; }
    }
    ready = true; paint();
    if (hasCallback || location.hash === '#account') { const savedError = errorKey; open(); errorKey = savedError; paint(); }
  }
  document.addEventListener('click', event => {
    const trigger = event.target.closest?.('[data-auth-open]');
    if (trigger) { event.preventDefault(); open(trigger); return; }
    if (event.target.closest?.('[data-auth-logout]')) { event.preventDefault(); void logout(); }
  });
  new MutationObserver(paint).observe(document.documentElement, { attributes: true, attributeFilter: ['lang'] });
  for (const event of ['languagechange', 'soundflow:languagechange', 'soundflow:language-change']) document.addEventListener(event, paint);
  document.addEventListener('visibilitychange', () => { if (!document.hidden && session && session.expires_at * 1000 <= Date.now() + 60000) void refresh().catch(() => {}); });
  window.SoundflowAuth = Object.freeze({ open, logout, getState: () => ({ authenticated: Boolean(user), email: user?.email || null, admin: user ? admin : null }) });
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', () => void initialize(), { once: true });
  else void initialize();
})();
