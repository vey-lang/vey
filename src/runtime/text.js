// The helpers for text.
function veyText(ctx) {
  var toList = ctx.toList;
  var utf8Encode = ctx.utf8Encode;
  var utf8Decode = ctx.utf8Decode;
  var toBytes = ctx.toBytes;

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

  // The helpers that change the shape of a text.
  var text = {
    capitalize: function (value) {
      var source = String(value);
      if (!source) return source;
      return source.charAt(0).toUpperCase() + source.slice(1);
    },
    // A text that may be used in an address, such as "hello-world".
    slug: function (value) {
      return String(value)
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-+|-+$/g, "");
    },
    // A name in the way of the code, such as "helloWorld".
    camel: function (value) {
      var words = String(value).split(/[^a-zA-Z0-9]+/).filter(Boolean);
      return words
        .map(function (word, index) {
          var lower = word.toLowerCase();
          if (index === 0) return lower;
          return lower.charAt(0).toUpperCase() + lower.slice(1);
        })
        .join("");
    },
    // A name in the way of an address, such as "hello-world".
    kebab: function (value) {
      return String(value)
        .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
        .replace(/[^a-zA-Z0-9]+/g, "-")
        .replace(/^-+|-+$/g, "")
        .toLowerCase();
    },
    padStart: function (value, length, fill) {
      return pad(String(value), length, fill === undefined ? " " : fill, true);
    },
    padEnd: function (value, length, fill) {
      return pad(String(value), length, fill === undefined ? " " : fill, false);
    },
    repeat: function (value, times) {
      var count = Number(times);
      if (!isFinite(count) || count <= 0) return "";
      return String(value).repeat(Math.floor(count));
    },
    reverse: function (value) {
      return String(value).split("").reverse().join("");
    },
    // The words of a text.
    words: function (value) {
      return String(value).split(/\s+/).filter(Boolean);
    },
    // A text with the indent that all of its lines share taken away.
    //
    // The first line is left alone, because it usually starts right behind
    // the quote of the text.
    dedent: function (value) {
      var lines = String(value).split("\n");
      var indent = null;
      lines.forEach(function (line, index) {
        if (index === 0 || !line.trim()) return;
        var spaces = line.length - line.replace(/^\s+/, "").length;
        if (indent === null || spaces < indent) indent = spaces;
      });
      if (!indent) return String(value);
      return lines
        .map(function (line, index) {
          if (index === 0) return line;
          return line.slice(indent);
        })
        .join("\n");
    },
    // The bytes of a text, so that the binary of a program is a list.
    encode: function (value) {
      return utf8Encode(value);
    },
    // The text of the bytes.
    decode: function (value) {
      return utf8Decode(toBytes(value));
    }
  };

  function pad(value, length, fill, before) {
    var size = Number(length);
    if (!isFinite(size) || size <= value.length) return value;
    var filler = String(fill);
    if (!filler) return value;
    var out = "";
    while (out.length < size - value.length) out += filler;
    out = out.slice(0, size - value.length);
    return before ? out + value : value + out;
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
  ctx.text = text;

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
    text: text,
  };
}
