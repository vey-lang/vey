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
  var sessions = Object.create(null);
  var metadata = Object.create(null);
  var settings = {
    ttl: 24 * 60 * 60 * 1000,
    idleTimeout: 30 * 60 * 1000,
    maxSessions: 10000
  };

  function remove(id) {
    delete sessions[id];
    delete metadata[id];
  }

  function expired(record, now) {
    return (settings.ttl > 0 && now - record.createdAt >= settings.ttl) ||
      (settings.idleTimeout > 0 && now - record.lastAccess >= settings.idleTimeout);
  }

  function cleanup() {
    var now = Date.now();
    for (var id in metadata) {
      if (expired(metadata[id], now)) remove(id);
    }
    var ids = Object.keys(metadata);
    while (settings.maxSessions > 0 && ids.length > settings.maxSessions) {
      var oldest = ids.shift();
      var oldestTime = metadata[oldest] ? metadata[oldest].lastAccess : Infinity;
      for (var i = 0; i < ids.length; i++) {
        var candidate = metadata[ids[i]];
        if (candidate && candidate.lastAccess < oldestTime) {
          oldest = ids[i];
          oldestTime = candidate.lastAccess;
        }
      }
      remove(oldest);
      ids = Object.keys(metadata);
    }
  }

  var cleanupTimer = null;
  if (isNode && typeof setInterval === "function") {
    cleanupTimer = setInterval(cleanup, 60000);
    if (cleanupTimer && cleanupTimer.unref) cleanupTimer.unref();
  }

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

  // The session begins and gets a name of its own.
  function begin(store) {
    store.id = newId();
    store.data = Object.create(null);
    sessions[store.id] = store.data;
    metadata[store.id] = { createdAt: Date.now(), lastAccess: Date.now() };
    cleanup();
    store.started = true;
    return store.id;
  }

  var session = {
    // The session begins, and the answer carries its name.
    start: function () {
      var store = current();
      if (!store) {
        throw veyError("session.start() is only available while a request is handled.");
      }
      if (!store.id) return begin(store);
      return store.id;
    },

    // The session gets a new name, and the old one is forgotten.
    //
    // A name that a visitor knows may not be used again, so that a
    // visitor who takes the name of another one cannot step into the
    // session of that other one.
    regenerate: function () {
      var store = current();
      if (!store) {
        throw veyError("session.regenerate() is only available while a request is handled.");
      }
      if (store.id) remove(store.id);
      return begin(store);
    },

    // The value of a key of the session.
    get: function (key) {
      var store = current();
      if (!store || !store.data) return null;
      if (metadata[store.id]) metadata[store.id].lastAccess = Date.now();
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
      if (metadata[store.id]) metadata[store.id].lastAccess = Date.now();
      return value;
    },

    // The session ends, and the answer forgets its name.
    end: function () {
      var store = current();
      if (!store || !store.id) return;
      remove(store.id);
      store.id = null;
      store.data = null;
      store.ended = true;
    },

    // The name of the session.
    id: function () {
      var store = current();
      return store && store.id ? store.id : null;
    },

    configure: function (options) {
      var opts = options || {};
      if (opts.ttl !== undefined) settings.ttl = Math.max(0, Number(opts.ttl));
      if (opts.idleTimeout !== undefined) settings.idleTimeout = Math.max(0, Number(opts.idleTimeout));
      if (opts.maxSessions !== undefined) settings.maxSessions = Math.max(0, Math.floor(Number(opts.maxSessions)));
      cleanup();
      return this;
    }
  };

  var sessions_ = {
    // The name of the cookie of the session.
    cookie: COOKIE,

    // The session of a request, from the cookies of the request.
    //
    // The second argument says whether the request came over a secure
    // connection, so that the cookie of the answer may say so too.
    open: function (cookies, secure) {
      var id = cookies && cookies[COOKIE] ? String(cookies[COOKIE]) : null;
      cleanup();
      var data = id && sessions[id] ? sessions[id] : null;
      if (data && metadata[id]) metadata[id].lastAccess = Date.now();
      return {
        id: data ? id : null,
        data: data,
        started: false,
        ended: false,
        secure: !!secure
      };
    },

    // The work of a request is done with its session at hand.
    run: run,

    // The cookie of the answer, when the session has begun or ended.
    header: function (store) {
      if (!store) return null;
      if (store.ended) {
        return cookieHeader(COOKIE, "", {
          maxAge: 0,
          expires: new Date(0).toUTCString(),
          path: "/",
          httpOnly: true,
          sameSite: "Lax"
        });
      }
      if (store.started && store.id) {
        // The cookie is only sent over a secure connection when the
        // request came over one.
        return cookieHeader(COOKIE, store.id, {
          httpOnly: true,
          sameSite: "Lax",
          path: "/",
          secure: store.secure
        });
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
