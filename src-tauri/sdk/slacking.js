/*
 * Slacking Desk 插件 SDK（由壳提供，地址固定为 /__sdk__/slacking.js）
 * 用法：<script src="/__sdk__/slacking.js"></script>，然后使用全局对象 slacking。
 * 所有方法都返回 Promise；失败时 reject 一个 Error，带 code 字段（permission_denied 等）。
 */
(function () {
  if (window.slacking) return;
  var seq = 0;
  var pending = {};
  var listeners = {};

  function call(method, params) {
    return new Promise(function (resolve, reject) {
      var id = ++seq;
      pending[id] = { resolve: resolve, reject: reject };
      parent.postMessage({ __slacking: "call", id: id, method: method, params: params === undefined ? null : params }, "*");
    });
  }

  function applyTheme(theme) {
    document.documentElement.setAttribute("data-theme", theme);
  }

  window.addEventListener("message", function (e) {
    if (e.source !== parent) return;
    var m = e.data;
    if (!m || typeof m !== "object") return;
    if (m.__slacking === "res") {
      var p = pending[m.id];
      if (!p) return;
      delete pending[m.id];
      if (m.error) {
        var err = new Error(m.error.message);
        err.code = m.error.code;
        p.reject(err);
      } else {
        p.resolve(m.result);
      }
    } else if (m.__slacking === "ping") {
      // 壳的看门狗：后台页必须引入本 SDK，否则会被判定为无响应
      parent.postMessage({ __slacking: "pong" }, "*");
    } else if (m.__slacking === "event") {
      if (m.name === "theme.changed") applyTheme(m.data.theme);
      (listeners[m.name] || []).slice().forEach(function (fn) {
        try { fn(m.data); } catch (x) { console.error(x); }
      });
    }
  });

  window.slacking = {
    call: call,
    /** 监听壳的事件，目前有 theme.changed。返回取消监听的函数。 */
    on: function (name, fn) {
      (listeners[name] = listeners[name] || []).push(fn);
      return function () {
        listeners[name] = (listeners[name] || []).filter(function (f) { return f !== fn; });
      };
    },
    app: {
      /** { shellVersion, platform, language, theme, pluginId } */
      info: function () { return call("app.info"); },
    },
    storage: {
      get: function (key) { return call("storage.get", { key: key }); },
      set: function (key, value) { return call("storage.set", { key: key, value: value }); },
      remove: function (key) { return call("storage.remove", { key: key }); },
    },
    notify: {
      /** 需要 notify 权限。{ title, body? } */
      show: function (opts) { return call("notify.show", opts); },
    },
    backend: {
      /** 调用插件自己的后端进程 */
      call: function (method, params) { return call("backend.call", { method: method, params: params === undefined ? null : params }); },
    },
  };

  call("app.info").then(function (i) { applyTheme(i.theme); }, function () {});
})();
