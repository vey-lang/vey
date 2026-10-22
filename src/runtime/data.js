// JSON, math, time, base64 and URLs.
function veyData(ctx) {
  var isNode = ctx.isNode;
  var scope = ctx.scope;
  var safeDecode = ctx.safeDecode;

  var base64 = {
    encode: function (value) {
      var text = String(value);
      if (isNode) return Buffer.from(text, "utf8").toString("base64");
      return scope.btoa(unescape(encodeURIComponent(text)));
    },
    decode: function (value) {
      var text = String(value);
      if (isNode) return Buffer.from(text, "base64").toString("utf8");
      return decodeURIComponent(escape(scope.atob(text)));
    }
  };

  var url = {
    encode: function (value) {
      return encodeURIComponent(String(value));
    },
    decode: function (value) {
      return safeDecode(String(value));
    },
    parse: function (value) {
      var text = String(value);
      var parsed;
      try {
        parsed = new URL(text);
      } catch (err) {
        // Relative URLs are resolved against a local origin so that
        // `url.parse("/a?b=1")` works instead of throwing.
        parsed = new URL(text, "http://localhost/");
      }
      var query = {};
      parsed.searchParams.forEach(function (item, key) { query[key] = item; });
      return {
        href: parsed.href,
        protocol: parsed.protocol,
        host: parsed.host,
        pathname: parsed.pathname,
        search: parsed.search,
        hash: parsed.hash,
        query: query
      };
    }
  };

  var json = {
    parse: function (text) { return JSON.parse(text); },
    stringify: function (value, indent) {
      var text = JSON.stringify(value, null, indent === undefined ? 2 : indent);
      // JSON.stringify returns undefined for values it cannot encode; the
      // result is a string either way.
      return text === undefined ? "undefined" : text;
    }
  };

  var math = {
    abs: Math.abs,
    ceil: Math.ceil,
    floor: Math.floor,
    round: Math.round,
    min: Math.min,
    max: Math.max,
    sqrt: Math.sqrt,
    pow: Math.pow,
    random: Math.random,
    pi: Math.PI
  };

  var time = {
    now: function () { return Date.now(); },
    iso: function () { return new Date().toISOString(); }
  };

  ctx.json = json;
  ctx.math = math;
  ctx.time = time;
  ctx.base64 = base64;
  ctx.url = url;

  return {
    json: json,
    math: math,
    time: time,
    base64: base64,
    url: url,
  };
}
