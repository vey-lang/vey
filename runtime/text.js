// The helpers for text.
function veyText(ctx) {
  var toList = ctx.toList;

  function join(list, separator) {
    return toList(list).join(separator === undefined ? "" : separator);
  }

  function split(text, separator) {
    return String(text).split(separator === undefined ? "" : separator);
  }

  function upper(text) { return String(text).toUpperCase(); }

  function lower(text) { return String(text).toLowerCase(); }

  function trim(text) { return String(text).trim(); }

  function replace(text, search, next) {
    return String(text).split(search).join(next === undefined ? "" : next);
  }

  function startsWith(text, prefix) {
    return String(text).indexOf(String(prefix)) === 0;
  }

  function endsWith(text, suffix) {
    var value = String(text);
    var end = String(suffix);
    if (end.length > value.length) return false;
    return value.lastIndexOf(end) === value.length - end.length;
  }

  function includes(haystack, needle) {
    if (typeof haystack === "string") return haystack.indexOf(needle) !== -1;
    if (Array.isArray(haystack)) return haystack.indexOf(needle) !== -1;
    return false;
  }

  ctx.upper = upper;
  ctx.lower = lower;
  ctx.trim = trim;
  ctx.replace = replace;
  ctx.includes = includes;
  ctx.startsWith = startsWith;
  ctx.endsWith = endsWith;
  ctx.split = split;
  ctx.join = join;

  return {
    upper: upper,
    lower: lower,
    trim: trim,
    replace: replace,
    includes: includes,
    startsWith: startsWith,
    endsWith: endsWith,
    split: split,
    join: join,
  };
}
