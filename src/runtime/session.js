// The sessions of the server: a state that lives from one request to the
// next.
function veySession(ctx) {
  var isNode = ctx.isNode;
  var nodeModule = ctx.nodeModule;
  var veyError = ctx.veyError;
  var cookieHeader = ctx.cookieHeader;

  // The name of the cookie of the session.
  var COOKIE = "vey.sid";

  // The sessions that are open, by their name.
  var sessions = {};

  // The storage of the request, which keeps the session of a request with
  // its answer even when the handler waits for something.
  var storage = null;
  if (isNode) {
    try {
      var hooks = nodeModule("node:async_hooks");
      if (hooks && hooks.AsyncLocalStorage) storage = new hooks.AsyncLocalStorage();
    } catch (err) {
      storage = null;
    }
  }
  // The store of the request, when the host has no storage of its own.
  var fallback = null;

  function current() {
    if (storage) return storage.getStore();
    return fallback;
  }

  // The name of a new session.
  function newId() {
    if (isNode) {
      try {
        return nodeModule("node:crypto").randomBytes(16).toString("hex");
      } catch (err) {
        // The host has no random bytes, so the name is made of them.
      }
    }
    var out = "";
    for (var i = 0; i < 32; i++) {
      out += Math.floor(Math.random() * 16).toString(16);
    }
    return out;
  }

  // The work of a request is done with its session at hand.
  function run(store, fn) {
    if (storage) return storage.run(store, fn);
    fallback = store;
    try {
      return fn();
    } finally {
      fallback = null;
    }
  }

  var session = {
    // The session begins, and the answer carries its name.
    start: function () {
      var store = current();
      if (!store) {
        throw veyError("session.start() is only available while a request is handled.");
      }
      if (!store.id) {
        store.id = newId();
        store.data = {};
        sessions[store.id] = store.data;
        store.started = true;
      }
      return store.id;
    },

    // The value of a key of the session.
    get: function (key) {
      var store = current();
      if (!store || !store.data) return null;
      var value = store.data[String(key)];
      return value === undefined ? null : value;
    },

    // The value of a key of the session is written.
    set: function (key, value) {
      var store = current();
      if (!store) {
        throw veyError("session.set() is only available while a request is handled.");
      }
      if (!store.id) session.start();
      store.data[String(key)] = value;
      return value;
    },

    // The session ends, and the answer forgets its name.
    end: function () {
      var store = current();
      if (!store || !store.id) return;
      delete sessions[store.id];
      store.id = null;
      store.data = null;
      store.ended = true;
    },

    // The name of the session.
    id: function () {
      var store = current();
      return store && store.id ? store.id : null;
    }
  };

  var sessions_ = {
    // The name of the cookie of the session.
    cookie: COOKIE,

    // The session of a request, from the cookies of the request.
    open: function (cookies) {
      var id = cookies && cookies[COOKIE] ? String(cookies[COOKIE]) : null;
      var data = id && sessions[id] ? sessions[id] : null;
      return { id: data ? id : null, data: data, started: false, ended: false };
    },

    // The work of a request is done with its session at hand.
    run: run,

    // The cookie of the answer, when the session has begun or ended.
    header: function (store) {
      if (!store) return null;
      if (store.ended) {
        return cookieHeader(COOKIE, "", { maxAge: 0, expires: new Date(0).toUTCString() });
      }
      if (store.started && store.id) {
        return cookieHeader(COOKIE, store.id, { httpOnly: true, sameSite: "Lax" });
      }
      return null;
    }
  };

  ctx.session = session;
  ctx.sessions = sessions_;

  return {
    session: session,
  };
}
