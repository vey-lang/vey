// The storage of the browser: the notes that stay after the page is closed.
function veyStorage(ctx) {
  var scope = ctx.scope;
  var doc = ctx.doc;
  var veyError = ctx.veyError;

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
        var store = storeOf(name);
        var value = store.getItem(String(key));
        return value === null || value === undefined ? null : value;
      },

      // The note of a key is written.
      set: function (key, value) {
        var store = storeOf(name);
        store.setItem(String(key), value === undefined || value === null ? "" : String(value));
      },

      // The note of a key is taken away.
      remove: function (key) {
        var store = storeOf(name);
        store.removeItem(String(key));
      },

      // Every note is taken away.
      clear: function () {
        var store = storeOf(name);
        store.clear();
      },

      // The names of the notes, in the order of the browser.
      keys: function () {
        var store = storeOf(name);
        var out = [];
        for (var i = 0; i < store.length; i++) {
          var key = store.key(i);
          if (key !== null && key !== undefined) out.push(key);
        }
        return out;
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
