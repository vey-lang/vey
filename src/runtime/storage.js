// The storage of the browser: the notes that stay after the page is closed.
function veyStorage(ctx) {
  var scope = ctx.scope;
  var doc = ctx.doc;
  var veyError = ctx.veyError;

  var memoryStores = Object.create(null);
  var memorySettings = { ttl: 0 };

  function memoryStore(name) {
    if (!memoryStores[name]) memoryStores[name] = Object.create(null);
    return memoryStores[name];
  }

  function cleanupMemory(store) {
    if (memorySettings.ttl <= 0) return;
    var now = Date.now();
    for (var key in store) {
      if (store[key].expiresAt > 0 && store[key].expiresAt <= now) delete store[key];
    }
  }

  // The store of the browser, when the page has one.
  function storeOf(name) {
    if (!doc) {
      throw veyError(name + " is only available in the browser.");
    }
    var view = doc.defaultView || scope;
    var store = view[name];
    if (!store) {
      throw veyError(name + " is not available in this browser.");
    }
    return store;
  }

  function makeStorage(name) {
    return {
      // The note of a key, or nothing when there is none.
      get: function (key) {
        if (!doc) {
          var memory = memoryStore(name);
          cleanupMemory(memory);
          var item = memory[String(key)];
          return item === undefined ? null : item.value;
        }
        var store = storeOf(name);
        var value = store.getItem(String(key));
        return value === null || value === undefined ? null : value;
      },

      // The note of a key is written.
      set: function (key, value) {
        if (!doc) {
          var memory = memoryStore(name);
          memory[String(key)] = {
            value: value === undefined || value === null ? "" : String(value),
            expiresAt: memorySettings.ttl > 0 ? Date.now() + memorySettings.ttl : 0
          };
          return;
        }
        var store = storeOf(name);
        store.setItem(String(key), value === undefined || value === null ? "" : String(value));
      },

      // The note of a key is taken away.
      remove: function (key) {
        if (!doc) {
          delete memoryStore(name)[String(key)];
          return;
        }
        var store = storeOf(name);
        store.removeItem(String(key));
      },

      // Every note is taken away.
      clear: function () {
        if (!doc) {
          memoryStores[name] = Object.create(null);
          return;
        }
        var store = storeOf(name);
        store.clear();
      },

      // The names of the notes, in the order of the browser.
      keys: function () {
        if (!doc) {
          var memory = memoryStore(name);
          cleanupMemory(memory);
          return Object.keys(memory);
        }
        var store = storeOf(name);
        var out = [];
        for (var i = 0; i < store.length; i++) {
          var key = store.key(i);
          if (key !== null && key !== undefined) out.push(key);
        }
        return out;
      },

      configure: function (options) {
        if (doc) return this;
        var opts = options || {};
        if (opts.ttl !== undefined) memorySettings.ttl = Math.max(0, Number(opts.ttl));
        cleanupMemory(memoryStore(name));
        return this;
      }
    };
  }

  // The notes that stay after the browser is closed.
  var storage = makeStorage("localStorage");

  // The notes that stay until the tab of the browser is closed.
  var sessionStorage = makeStorage("sessionStorage");

  ctx.storage = storage;
  ctx.sessionStorage = sessionStorage;

  return {
    storage: storage,
    sessionStorage: sessionStorage,
  };
}
