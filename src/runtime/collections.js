// The helpers for lists and objects.
function veyCollections(ctx) {
  var ownProps = ctx.ownProps;
  var toList = ctx.toList;

  function append(list) {
    var result = toList(list).slice();
    for (var i = 1; i < arguments.length; i++) result.push(arguments[i]);
    return result;
  }

  function slice(value, start, end) {
    return (value || "").slice(start === undefined ? 0 : start, end);
  }

  function find(list, fn) {
    return toList(list).find(function (item, index) { return fn(item, index); });
  }

  function defaultCompare(a, b) {
    if (typeof a === "number" && typeof b === "number") return a - b;
    var left = String(a);
    var right = String(b);
    if (left < right) return -1;
    if (left > right) return 1;
    return 0;
  }

  function sort(list, fn) {
    var copy = toList(list).slice();
    return fn ? copy.sort(fn) : copy.sort(defaultCompare);
  }

  function map(list, fn) {
    return toList(list).map(function (item, index) { return fn(item, index); });
  }

  function filter(list, fn) {
    return toList(list).filter(function (item, index) { return fn(item, index); });
  }

  function reduce(list, fn, initial) {
    var items = toList(list);
    if (arguments.length < 3) {
      if (items.length === 0) {
        throw new Error("reduce() needs an initial value for an empty list");
      }
      return items.reduce(function (a, b, i) { return fn(a, b, i); });
    }
    return items.reduce(function (a, b, i) { return fn(a, b, i); }, initial);
  }

  function keys(object) { return Object.keys(ownProps(object)); }

  function omit(object, names) {
    var out = {};
    var source = ownProps(object);
    for (var key in source) {
      if (names.indexOf(key) === -1) out[key] = source[key];
    }
    return out;
  }

  function values(object) {
    var props = ownProps(object);
    return Object.keys(props).map(function (key) { return props[key]; });
  }

  function entries(object) { return Object.entries(ownProps(object)); }

  function range(start, end) {
    if (end === undefined) { end = start; start = 0; }
    var out = [];
    for (var i = start; i < end; i++) out.push(i);
    return out;
  }

  function flatMap(list, fn) {
    var out = [];
    var items = toList(list);
    for (var i = 0; i < items.length; i++) {
      var value = fn(items[i], i);
      if (Array.isArray(value)) {
        for (var j = 0; j < value.length; j++) {
          if (value[j] !== null && value[j] !== undefined) out.push(value[j]);
        }
      } else if (value !== null && value !== undefined) {
        out.push(value);
      }
    }
    return out;
  }

  function sum(list) {
    var items = list || [];
    var total = 0;
    for (var i = 0; i < items.length; i++) total += Number(items[i]);
    return total;
  }

  function unique(list) {
    return Array.from(new Set(toList(list)));
  }

  function reverse(list) {
    return toList(list).slice().reverse();
  }

  // A copy of a value that holds nothing of the value it came from.
  //
  // The values that cannot be written down, such as a class of the host,
  // are left as they are.
  function deepCopy(value, seen) {
    if (value === null || typeof value !== "object") return value;
    // The values of an enum never change, so they are shared.
    if (value.__veyEnum) return value;
    var done = seen || [];
    for (var i = 0; i < done.length; i++) {
      if (done[i][0] === value) return done[i][1];
    }
    if (value instanceof Date) return new Date(value.getTime());
    if (Array.isArray(value)) {
      var list = [];
      done.push([value, list]);
      for (var j = 0; j < value.length; j++) list.push(deepCopy(value[j], done));
      return list;
    }
    if (Object.prototype.toString.call(value) !== "[object Object]") return value;
    var out = {};
    done.push([value, out]);
    for (var key in value) {
      if (Object.prototype.hasOwnProperty.call(value, key)) {
        out[key] = deepCopy(value[key], done);
      }
    }
    return out;
  }

  ctx.append = append;
  ctx.slice = slice;
  ctx.find = find;
  ctx.sort = sort;
  ctx.map = map;
  ctx.filter = filter;
  ctx.reduce = reduce;
  ctx.keys = keys;
  ctx.omit = omit;
  ctx.values = values;
  ctx.entries = entries;
  ctx.range = range;
  ctx.flatMap = flatMap;
  ctx.sum = sum;
  ctx.unique = unique;
  ctx.reverse = reverse;
  ctx.deepCopy = deepCopy;

  return {
    append: append,
    slice: slice,
    find: find,
    sort: sort,
    map: map,
    filter: filter,
    reduce: reduce,
    keys: keys,
    omit: omit,
    values: values,
    entries: entries,
    range: range,
    flatMap: flatMap,
    sum: sum,
    unique: unique,
    reverse: reverse,
    deepCopy: deepCopy,
  };
}
