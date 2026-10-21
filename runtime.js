function createVeyRuntime(scope, req) {
  scope = scope || globalThis;
  req = req || null;

  // Node is detected from the `require` hook that the CLI passes in, not from
  // the ambient `process` global: bundlers and test runners often define a
  // `process` shim in the browser, which would wrongly unlock Node-only
  // helpers such as `fs` and `path`.
  var isNode = typeof req === "function";
  var doc = scope.document || null;
  var log = scope.console || console;

  function flatten(list, out) {
    out = out || [];
    for (var i = 0; i < list.length; i++) {
      var item = list[i];
      if (Array.isArray(item)) {
        flatten(item, out);
      } else if (item !== null && item !== undefined && item !== false && item !== true) {
        out.push(item);
      }
    }
    return out;
  }

  /** Own enumerable properties of `value`, without inherited noise. */
  function ownProps(value) {
    var out = {};
    if (value === null || value === undefined) return out;
    for (var key in value) {
      if (Object.prototype.hasOwnProperty.call(value, key)) out[key] = value[key];
    }
    return out;
  }

  /** `value` as a list, accepting a string as a list of characters. */
  function toList(value) {
    if (value === null || value === undefined) return [];
    if (Array.isArray(value)) return value;
    if (typeof value === "string") return value.split("");
    return [];
  }

  /** `decodeURIComponent` that keeps malformed input instead of throwing. */
  function safeDecode(value) {
    try {
      return decodeURIComponent(value);
    } catch (err) {
      return value;
    }
  }

  /** `value` as text, encoding objects as JSON instead of "[object Object]". */
  function toText(value) {
    if (typeof value === "string") return value;
    if (value === null || value === undefined) return "";
    if (typeof value === "object") {
      try {
        return JSON.stringify(value, null, 2);
      } catch (err) {
        return String(value);
      }
    }
    return String(value);
  }

  /** `backgroundColor` -> `background-color`, leaving custom properties alone. */
  function cssPropertyName(name) {
    if (name.indexOf("--") === 0) return name;
    return name.replace(/[A-Z]/g, function (ch) { return "-" + ch.toLowerCase(); });
  }

  /** `background-color` -> `backgroundColor`, leaving custom properties alone. */
  function domStyleName(name) {
    if (name.indexOf("--") === 0) return name;
    return name.replace(/-([a-z])/g, function (_, ch) { return ch.toUpperCase(); });
  }

  /** An error that belongs to the program, not to the runtime. */
  function veyError(message) {
    var error = new Error(message);
    error.vey = true;
    return error;
  }

  function escapeHtml(value) {
    return String(value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  // A null prototype keeps inherited names such as "constructor" from
  // looking like void tags.
  var VOID_TAGS = Object.assign(Object.create(null), {
    area: 1, base: 1, br: 1, col: 1, embed: 1, hr: 1, img: 1, input: 1,
    link: 1, meta: 1, param: 1, source: 1, track: 1, wbr: 1
  });

  function h(tag, props, children, args) {
    var kids = flatten(children || []);
    var attrs = props || {};

    if (typeof tag === "function") {
      // Components receive a marked props object so that a plain positional
      // call (`Card("title")`) stays distinguishable from a named one
      // (`Card title="title"`), even when the first argument is an object.
      var named = Object.assign(Object.create(null), attrs);
      // The marker is hidden, so that it does not show up in the props.
      Object.defineProperty(named, "__veyProps", { value: true, enumerable: false });
      if (kids.length) named.children = kids;
      var callArgs = [named];
      if (args) {
        for (var i = 0; i < args.length; i++) callArgs.push(args[i]);
      }
      return tag.apply(null, callArgs);
    }

    if (doc) {
      var el = doc.createElement(tag);
      applyProps(el, attrs);
      for (var j = 0; j < kids.length; j++) el.appendChild(toDom(kids[j]));
      return el;
    }

    return { __vey: true, tag: tag, props: attrs, children: kids };
  }

  function applyProps(el, attrs) {
    var props = ownProps(attrs);
    for (var key in props) {
      var value = props[key];
      if (value === null || value === undefined || value === false) continue;
      if (key.indexOf("on") === 0 && typeof value === "function") {
        el.addEventListener(key.slice(2).toLowerCase(), value);
        continue;
      }
      if (key === "class") {
        el.setAttribute("class", value);
        continue;
      }
      if (key === "style" && typeof value === "object") {
        var styles = ownProps(value);
        for (var name in styles) {
          var prop = domStyleName(name);
          if (prop.indexOf("--") === 0) el.style.setProperty(prop, styles[name]);
          else el.style[prop] = styles[name];
        }
        continue;
      }
      if (key === "value" || key === "checked") {
        el[key] = value;
        continue;
      }
      el.setAttribute(key, value === true ? "" : String(value));
    }
  }

  function toDom(value) {
    if (value === null || value === undefined) return doc.createTextNode("");
    if (typeof value === "string" || typeof value === "number") {
      return doc.createTextNode(String(value));
    }
    if (Array.isArray(value)) {
      var fragment = doc.createDocumentFragment();
      for (var i = 0; i < value.length; i++) fragment.appendChild(toDom(value[i]));
      return fragment;
    }
    if (value.nodeType) return value;
    if (value.__veyRaw) {
      var template = doc.createElement("template");
      template.innerHTML = String(value.value);
      return template.content;
    }
    if (value.__vey) {
      var el = doc.createElement(value.tag);
      applyProps(el, value.props || {});
      for (var j = 0; j < value.children.length; j++) {
        el.appendChild(toDom(value.children[j]));
      }
      return el;
    }
    return doc.createTextNode(String(value));
  }

  function html(node) {
    if (node === null || node === undefined || node === false || node === true) return "";
    if (Array.isArray(node)) {
      var out = "";
      for (var i = 0; i < node.length; i++) out += html(node[i]);
      return out;
    }
    if (node.__veyRaw) return String(node.value);
    if (typeof node === "string") return escapeHtml(node);
    if (typeof node === "number") return String(node);
    if (node.nodeType) return node.outerHTML || "";
    if (!node.__vey) return escapeHtml(node);

    var attrs = "";
    var props = ownProps(node.props || {});
    for (var key in props) {
      var value = props[key];
      if (value === null || value === undefined || value === false) continue;
      if (key.indexOf("on") === 0 || typeof value === "function") continue;
      if (key === "style" && typeof value === "object") {
        var styles = ownProps(value);
        var css = "";
        for (var name in styles) css += cssPropertyName(name) + ": " + styles[name] + "; ";
        attrs += " style=\"" + escapeHtml(css) + "\"";
        continue;
      }
      if (value === true) {
        attrs += " " + key;
      } else {
        attrs += " " + key + "=\"" + escapeHtml(value) + "\"";
      }
    }

    if (VOID_TAGS[node.tag]) return "<" + node.tag + attrs + ">";
    var children = "";
    for (var j = 0; j < node.children.length; j++) children += html(node.children[j]);
    return "<" + node.tag + attrs + ">" + children + "</" + node.tag + ">";
  }

  function raw(value) {
    return {
      __veyRaw: true,
      value: value === null || value === undefined ? "" : String(value)
    };
  }

  function bind(target, node) {
    if (!doc) {
      throw veyError(
        "bind() is only available in the browser. Open the page with `vey dev` " +
          "or build it with `vey web`."
      );
    }
    var el = typeof target === "string" ? doc.querySelector(target) : target;
    if (!el) throw new Error("element not found: " + target);
    var nodes = flatten([node]).map(toDom);
    el.replaceChildren.apply(el, nodes);
    return el;
  }

  function cls() {
    var parts = [];
    for (var i = 0; i < arguments.length; i++) {
      var value = arguments[i];
      if (value) parts.push(String(value));
    }
    return parts.join(" ");
  }

  function enumValue(enumName, variant) {
    var value = {};
    // The bookkeeping fields stay hidden so that they do not show up in
    // `keys`, `values` or `entries`.
    Object.defineProperty(value, "__veyEnum", { value: enumName, enumerable: false });
    Object.defineProperty(value, "__veyVariant", { value: variant, enumerable: false });
    Object.defineProperty(value, "toString", {
      value: function () {
        return enumName + "." + variant;
      },
      enumerable: false
    });
    return Object.freeze(value);
  }

  function enumCtor(enumName, variant, fields) {
    return function () {
      var args = Array.prototype.slice.call(arguments);
      var value = {};
      Object.defineProperty(value, "__veyEnum", { value: enumName, enumerable: false });
      Object.defineProperty(value, "__veyVariant", { value: variant, enumerable: false });
      for (var i = 0; i < fields.length; i++) value[fields[i]] = args[i];
      Object.defineProperty(value, "toString", {
        value: function () {
          var parts = [];
          for (var j = 0; j < fields.length; j++) {
            parts.push(fields[j] + "=" + str(value[fields[j]]));
          }
          return variant + "(" + parts.join(", ") + ")";
        },
        enumerable: false
      });
      return Object.freeze(value);
    };
  }

  function print() {
    var args = Array.prototype.slice.call(arguments).map(function (value) {
      return value && value.__veyEnum ? value.toString() : value;
    });
    log.log.apply(log, args);
  }

  function input(prompt) {
    if (!isNode) throw veyError("input() is only available in Node.");
    if (prompt) process.stdout.write(String(prompt));
    var fs = nodeModule("node:fs");
    // Read one byte at a time so that only the current line is consumed and
    // the rest of stdin stays available for the next call.
    var one = Buffer.alloc(1);
    var bytes = [];
    for (;;) {
      var count = fs.readSync(0, one, 0, 1, null);
      if (count === 0) break;
      var ch = one.toString("utf8", 0, 1);
      if (ch === "\n") break;
      bytes.push(one[0]);
    }
    var line = Buffer.from(bytes).toString("utf8");
    return line.replace(/\r$/, "");
  }

  function sleep(ms) {
    return new Promise(function (resolve) { setTimeout(resolve, ms); });
  }

  function str(value) {
    if (value === null) return "null";
    if (value === undefined) return "undefined";
    if (value && value.__veyEnum) return value.toString();
    if (typeof value === "object") {
      try {
        return JSON.stringify(value, null, 2);
      } catch (err) {
        return Object.prototype.toString.call(value);
      }
    }
    return String(value);
  }

  function num(value) { return Number(value); }
  function int(value) { return parseInt(value, 10); }
  function bool(value) { return Boolean(value); }

  function type(value) {
    if (value === null) return "null";
    if (value === undefined) return "undefined";
    if (value && value.__veyEnum) return "enum";
    if (Array.isArray(value)) return "list";
    var kind = typeof value;
    if (kind === "number") return "number";
    if (kind === "string") return "string";
    if (kind === "boolean") return "boolean";
    if (kind === "function") return "function";
    return "object";
  }

  function len(value) {
    if (value === null || value === undefined) return 0;
    if (typeof value === "string" || Array.isArray(value)) return value.length;
    return Object.keys(value).length;
  }

  function append(list) {
    var result = toList(list).slice();
    for (var i = 1; i < arguments.length; i++) result.push(arguments[i]);
    return result;
  }

  function slice(value, start, end) {
    return (value || "").slice(start === undefined ? 0 : start, end);
  }

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

  function find(list, fn) {
    return toList(list).find(function (item, index) { return fn(item, index); });
  }

  /** Default order: numbers compare numerically, everything else as text. */
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

  /** The object without the given keys. */
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

  function error(message) {
    throw new Error(String(message));
  }

  function assert(condition, message) {
    if (!condition) {
      throw new Error(message === undefined ? "assertion failed" : String(message));
    }
  }

  var pendingTests = 0;
  var testKeepAlive = null;

  function startTestKeepAlive() {
    if (!isNode || testKeepAlive) return;
    testKeepAlive = setInterval(function () {}, 1000);
  }

  function finishTest() {
    pendingTests -= 1;
    if (pendingTests <= 0 && testKeepAlive) {
      clearInterval(testKeepAlive);
      testKeepAlive = null;
    }
  }

  function test(name, fn) {
    try {
      var result = fn();
      if (result && typeof result.then === "function") {
        pendingTests += 1;
        startTestKeepAlive();
        return result.then(
          function () {
            try {
              log.log("ok " + name);
            } finally {
              finishTest();
            }
          },
          function (err) {
            try {
              failTest(name, err);
            } finally {
              finishTest();
            }
          }
        );
      }
      log.log("ok " + name);
    } catch (err) {
      failTest(name, err);
    }
  }

  function failTest(name, err) {
    log.error("fail " + name);
    log.error(testFailure(err));
    if (isNode && typeof process !== "undefined") process.exitCode = 1;
  }

  /// The message of a failed test, with the place it happened.
  ///
  /// The whole stack of Node is not shown, because most of it belongs to the
  /// runtime and not to the program.
  function testFailure(err) {
    if (!err) return "unknown error";
    var message = err && err.message !== undefined ? String(err.message) : String(err);
    var stack = err && err.stack ? String(err.stack) : "";
    var place = /\(([^()]*\.vey:\d+:\d+)\)/.exec(stack);
    if (place) {
      return message + "\n  " + place[1];
    }
    return message;
  }

  function all(list) {
    return Promise.all(list || []);
  }

  function race(list) {
    return Promise.race(list || []);
  }

  function env(name, fallback) {
    var value = isNode && typeof process !== "undefined" && process.env
      ? process.env[name]
      : undefined;
    if (value === undefined) {
      return fallback === undefined ? null : fallback;
    }
    return value;
  }

  function uuid() {
    if (isNode) {
      var cryptoModule = nodeModule("node:crypto");
      if (cryptoModule.randomUUID) return cryptoModule.randomUUID();
    }
    if (scope.crypto && scope.crypto.randomUUID) return scope.crypto.randomUUID();
    return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, function (ch) {
      var value = Math.random() * 16 | 0;
      var nibble = ch === "x" ? value : (value & 0x3 | 0x8);
      return nibble.toString(16);
    });
  }

  function random(min, max) {
    if (min === undefined && max === undefined) return Math.random();
    if (max === undefined) { max = min; min = 0; }
    return Math.floor(Math.random() * (max - min + 1)) + min;
  }

  function clamp(value, min, max) {
    return Math.min(Math.max(value, min), max);
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

  function nodeModule(name) {
    if (!isNode) throw veyError(name + " is only available in Node.");
    return req(name);
  }

  var fs = {
    read: function (path, encoding) {
      return nodeModule("node:fs").readFileSync(path, encoding || "utf8");
    },
    write: function (path, value) {
      nodeModule("node:fs").writeFileSync(path, toText(value), "utf8");
    },
    append: function (path, value) {
      nodeModule("node:fs").appendFileSync(path, toText(value), "utf8");
    },
    exists: function (path) {
      return nodeModule("node:fs").existsSync(path);
    },
    list: function (path) {
      return nodeModule("node:fs").readdirSync(path);
    },
    remove: function (path) {
      nodeModule("node:fs").rmSync(path, { recursive: true, force: true });
    },
    mkdir: function (path) {
      nodeModule("node:fs").mkdirSync(path, { recursive: true });
    }
  };

  var path = {
    join: function () {
      var parts = Array.prototype.slice.call(arguments);
      return nodeModule("node:path").join.apply(null, parts);
    },
    dirname: function (value) {
      return nodeModule("node:path").dirname(value);
    },
    basename: function (value) {
      return nodeModule("node:path").basename(value);
    },
    extname: function (value) {
      return nodeModule("node:path").extname(value);
    },
    resolve: function () {
      var parts = Array.prototype.slice.call(arguments);
      return nodeModule("node:path").resolve.apply(null, parts);
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

  function request(url, options) {
    var fetchFn = scope.fetch || (typeof fetch !== "undefined" ? fetch : null);
    if (!fetchFn) return Promise.reject(new Error("fetch is not available"));
    return fetchFn(url, options).then(function (response) {
      return response.text().then(function (text) {
        var contentType = response.headers.get("content-type") || "";
        var body = text;
        if (contentType.indexOf("json") !== -1) {
          try { body = JSON.parse(text); } catch (err) { body = text; }
        }
        var headers = {};
        response.headers.forEach(function (value, key) { headers[key] = value; });
        return {
          status: response.status,
          ok: response.ok,
          body: body,
          text: text,
          headers: headers
        };
      });
    });
  }

  var http = {
    get: function (url, options) {
      return request(url, options);
    },
    post: function (url, body, options) {
      // Copy the caller's options so that the request never mutates them.
      var opts = Object.assign({}, options || {});
      var headers = Object.assign({}, opts.headers || {});
      headers["content-type"] = headers["content-type"] || "application/json";
      opts.headers = headers;
      opts.method = "POST";
      opts.body = typeof body === "string" ? body : JSON.stringify(body);
      return request(url, opts);
    }
  };

  function matchRoute(pattern, pathname) {
    var patternParts = pattern.split("/").filter(Boolean);
    var pathParts = pathname.split("/").filter(Boolean);
    var wildcard = patternParts.indexOf("*");
    if (wildcard === -1) {
      if (patternParts.length !== pathParts.length) return null;
    } else if (pathParts.length < wildcard) {
      return null;
    }
    var params = {};
    var end = wildcard === -1 ? patternParts.length : wildcard;
    for (var i = 0; i < end; i++) {
      var part = patternParts[i];
      if (part.charAt(0) === "{" && part.charAt(part.length - 1) === "}") {
        params[part.slice(1, -1)] = safeDecode(pathParts[i]);
      } else if (part !== pathParts[i]) {
        return null;
      }
    }
    if (wildcard !== -1) {
      // A wildcard of "/*" only matches when there is something after the
      // slash, so that it does not swallow the root path.
      if (wildcard === 0 && patternParts.length === 1 && pathParts.length === 0) {
        return null;
      }
      params.splat = safeDecode(pathParts.slice(wildcard).join("/"));
    }
    return params;
  }

  function parseCookies(header) {
    var out = {};
    if (!header) return out;
    var parts = header.split(";");
    for (var i = 0; i < parts.length; i++) {
      var index = parts[i].indexOf("=");
      if (index === -1) continue;
      out[parts[i].slice(0, index).trim()] = safeDecode(parts[i].slice(index + 1).trim());
    }
    return out;
  }

  var MIME = Object.assign(Object.create(null), {
    ".html": "text/html; charset=utf-8",
    ".css": "text/css; charset=utf-8",
    ".js": "text/javascript; charset=utf-8",
    ".json": "application/json; charset=utf-8",
    ".svg": "image/svg+xml",
    ".png": "image/png",
    ".jpg": "image/jpeg",
    ".jpeg": "image/jpeg",
    ".gif": "image/gif",
    ".webp": "image/webp",
    ".ico": "image/x-icon",
    ".txt": "text/plain; charset=utf-8",
    ".woff2": "font/woff2",
    ".mp4": "video/mp4",
    ".mp3": "audio/mpeg"
  });

  function server(port) {
    if (!req) throw veyError("server() is only available in Node.");
    var httpModule = req("node:http");
    var fs = req("node:fs");
    var pathModule = req("node:path");
    var routes = [];
    var staticDir = null;
    var notFoundHandler = null;
    var actualPort = port === undefined ? 3000 : port;

    function add(method, pattern, handler) {
      routes.push({ method: method, pattern: pattern, handler: handler });
    }

    function readBody(request) {
      return new Promise(function (resolve, reject) {
        var chunks = [];
        request.on("data", function (chunk) { chunks.push(chunk); });
        request.on("end", function () {
          var raw = Buffer.concat(chunks).toString("utf8");
          var parsed = raw;
          var contentType = request.headers["content-type"] || "";
          if (contentType.indexOf("json") !== -1 && raw.trim()) {
            try { parsed = JSON.parse(raw); } catch (err) { parsed = raw; }
          }
          resolve({ raw: raw, parsed: parsed });
        });
        request.on("error", reject);
      });
    }

    function send(response, result, fallbackStatus) {
      if (result === undefined || result === null) {
        // A notFound handler may legitimately return nothing; the request
        // should still be reported as not found rather than as "no content".
        response.writeHead(fallbackStatus === undefined ? 204 : fallbackStatus);
        response.end();
        return;
      }
      var status = fallbackStatus === undefined ? 200 : fallbackStatus;
      var headers = {};
      var body = result;
      if (result && result.__veyResponse) {
        status = result.status;
        headers = result.headers || {};
        body = result.body;
      }
      if (body === undefined || body === null) body = "";
      if (typeof body === "string") {
        if (!headers["Content-Type"]) headers["Content-Type"] = "text/html; charset=utf-8";
      } else if (Buffer.isBuffer(body)) {
        if (!headers["Content-Type"]) headers["Content-Type"] = "application/octet-stream";
      } else {
        if (!headers["Content-Type"]) headers["Content-Type"] = "application/json; charset=utf-8";
        body = JSON.stringify(body);
      }
      response.writeHead(status, headers);
      response.end(body);
    }

    function serveStatic(pathname, response, status) {
      var relative = safeDecode(pathname).replace(/^\/+/, "");
      var file = pathModule.resolve(staticDir, relative);
      if (file !== staticDir && file.indexOf(staticDir + pathModule.sep) !== 0) {
        response.writeHead(403);
        response.end("Forbidden");
        return true;
      }
      try {
        var stat = fs.statSync(file);
        if (stat.isDirectory()) file = pathModule.join(file, "index.html");
        if (!fs.existsSync(file)) return false;
        var ext = pathModule.extname(file).toLowerCase();
        response.writeHead(status === undefined ? 200 : status, {
          "Content-Type": MIME[ext] || "application/octet-stream"
        });
        response.end(fs.readFileSync(file));
        return true;
      } catch (err) {
        return false;
      }
    }

    function makeRequest(request, url, params) {
      var query = {};
      url.searchParams.forEach(function (value, key) { query[key] = value; });
      return {
        path: url.pathname,
        method: request.method,
        params: params,
        query: query,
        headers: request.headers,
        body: null,
        text: "",
        cookies: parseCookies(request.headers.cookie)
      };
    }

    var serverInstance = httpModule.createServer(function (request, response) {
      // A malformed request must not take the whole process down.
      try {
        handleRequest(request, response);
      } catch (err) {
        log.error(err);
        try {
          response.writeHead(500, { "Content-Type": "text/plain; charset=utf-8" });
          response.end("Server error: " + (err && err.message ? err.message : err));
        } catch (ignored) {
          response.end();
        }
      }
    });

    function handleRequest(request, response) {
      var url = new URL(request.url || "/", "http://" + (request.headers.host || "localhost"));
      var handler = null;
      var params = {};
      for (var i = 0; i < routes.length; i++) {
        var route = routes[i];
        if (route.method !== request.method) continue;
        var matched = matchRoute(route.pattern, url.pathname);
        if (matched) {
          handler = route.handler;
          params = matched;
          break;
        }
      }

      if (!handler) {
        if (staticDir && serveStatic(url.pathname, response)) return;
        if (staticDir && serveStatic("/404.html", response, 404)) return;
        if (notFoundHandler) {
          Promise.resolve(notFoundHandler(makeRequest(request, url, params)))
            .then(function (result) { send(response, result, 404); })
            .catch(function (err) {
              log.error(err);
              response.writeHead(500, { "Content-Type": "text/plain; charset=utf-8" });
              response.end("Server error: " + (err && err.message ? err.message : err));
            });
          return;
        }
        response.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
        response.end("Not found");
        return;
      }

      readBody(request).then(function (body) {
        var info = makeRequest(request, url, params);
        info.body = body.parsed;
        info.text = body.raw;
        return Promise.resolve(handler(info)).then(function (result) {
          send(response, result);
        });
      }).catch(function (err) {
        log.error(err);
        response.writeHead(500, { "Content-Type": "text/plain; charset=utf-8" });
        response.end("Server error: " + (err && err.message ? err.message : err));
      });
    }

    return {
      get: function (pattern, handler) { add("GET", pattern, handler); return this; },
      post: function (pattern, handler) { add("POST", pattern, handler); return this; },
      put: function (pattern, handler) { add("PUT", pattern, handler); return this; },
      patch: function (pattern, handler) { add("PATCH", pattern, handler); return this; },
      del: function (pattern, handler) { add("DELETE", pattern, handler); return this; },
      route: function (method, pattern, handler) {
        add(String(method).toUpperCase(), pattern, handler);
        return this;
      },
      static: function (dir) {
        staticDir = pathModule.resolve(dir);
        return this;
      },
      notFound: function (handler) {
        notFoundHandler = handler;
        return this;
      },
      start: function (callback) {
        serverInstance.listen(actualPort, function () {
          var address = serverInstance.address();
          var port = address && address.port ? address.port : actualPort;
          print("Vey server listening on http://localhost:" + port);
          if (callback) callback();
        });
        return this;
      },
      stop: function () {
        serverInstance.close();
        return this;
      }
    };
  }

  function response(body, status, headers) {
    return {
      __veyResponse: true,
      body: body,
      status: status === undefined ? 200 : status,
      headers: headers || {}
    };
  }

  function redirect(location, status) {
    return {
      __veyResponse: true,
      body: "",
      status: status === undefined ? 302 : status,
      headers: { Location: location }
    };
  }

  return {
    h: h,
    html: html,
    raw: raw,
    bind: bind,
    cls: cls,
    enumValue: enumValue,
    enumCtor: enumCtor,
    print: print,
    input: input,
    sleep: sleep,
    str: str,
    num: num,
    int: int,
    bool: bool,
    type: type,
    len: len,
    append: append,
    slice: slice,
    join: join,
    split: split,
    upper: upper,
    lower: lower,
    trim: trim,
    replace: replace,
    includes: includes,
    startsWith: startsWith,
    endsWith: endsWith,
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
    error: error,
    assert: assert,
    test: test,
    all: all,
    race: race,
    env: env,
    uuid: uuid,
    random: random,
    clamp: clamp,
    sum: sum,
    unique: unique,
    reverse: reverse,
    base64: base64,
    url: url,
    fs: fs,
    path: path,
    json: json,
    math: math,
    time: time,
    http: http,
    server: server,
    response: response,
    redirect: redirect
  };
}
