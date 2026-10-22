// The maps and the sets.
//
// The keys of a map are not texts, and a set holds the values it has seen
// once. They are the same as the ones of JavaScript, so that a program may
// use them the way it uses the others.
function veyMap(ctx) {
  var toList = ctx.toList;

  // A map keeps the order in which the keys were put in it.
  function makeMap(entries) {
    var store = new Map();
    var map = {
      __veyMap: true,
      get size() { return store.size; }
    };

    map.set = function (key, value) {
      store.set(key, value);
      return map;
    };
    map.get = function (key) {
      // A key that is not there is `null`, the way the rest of the runtime
      // says it, and not `undefined`.
      return store.has(key) ? store.get(key) : null;
    };
    map.has = function (key) {
      return store.has(key);
    };
    map.delete = function (key) {
      return store.delete(key);
    };
    map.clear = function () {
      store.clear();
      return map;
    };
    map.keys = function () {
      return Array.from(store.keys());
    };
    map.values = function () {
      return Array.from(store.values());
    };
    // The pairs of the map, each one a list of two values.
    map.entries = function () {
      var out = [];
      store.forEach(function (value, key) { out.push([key, value]); });
      return out;
    };
    map.forEach = function (fn) {
      store.forEach(function (value, key) { fn(value, key, map); });
    };
    // The map is a list of its pairs when it is used as one.
    map.toList = function () {
      return map.entries();
    };

    if (entries) {
      var list = toList(entries);
      for (var i = 0; i < list.length; i++) {
        var pair = list[i];
        if (Array.isArray(pair)) map.set(pair[0], pair[1]);
      }
    }
    return map;
  }

  // A set holds the values it has seen once.
  function makeSet(values) {
    var store = new Set();
    var set = {
      __veySet: true,
      get size() { return store.size; }
    };

    set.add = function (value) {
      store.add(value);
      return set;
    };
    set.has = function (value) {
      return store.has(value);
    };
    set.delete = function (value) {
      return store.delete(value);
    };
    set.clear = function () {
      store.clear();
      return set;
    };
    set.values = function () {
      return Array.from(store.values());
    };
    // The values are a list when the set is used as one.
    set.toList = function () {
      return set.values();
    };
    set.forEach = function (fn) {
      store.forEach(function (value) { fn(value, value, set); });
    };

    if (values) {
      var list = toList(values);
      for (var i = 0; i < list.length; i++) store.add(list[i]);
    }
    return set;
  }

  ctx.Map = makeMap;
  ctx.Set = makeSet;

  return {
    Map: makeMap,
    Set: makeSet,
  };
}
