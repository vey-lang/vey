// The helpers that the other parts share.
function veyShared(ctx) {
  var scope = ctx.scope;

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

  function ownProps(value) {
    var out = {};
    if (value === null || value === undefined) return out;
    for (var key in value) {
      if (Object.prototype.hasOwnProperty.call(value, key)) out[key] = value[key];
    }
    return out;
  }

  function toList(value) {
    if (value === null || value === undefined) return [];
    if (Array.isArray(value)) return value;
    if (typeof value === "string") return value.split("");
    return [];
  }

  function safeDecode(value) {
    try {
      return decodeURIComponent(value);
    } catch (err) {
      return value;
    }
  }

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

  function cssPropertyName(name) {
    if (name.indexOf("--") === 0) return name;
    return name.replace(/[A-Z]/g, function (ch) { return "-" + ch.toLowerCase(); });
  }

  function domStyleName(name) {
    if (name.indexOf("--") === 0) return name;
    return name.replace(/-([a-z])/g, function (_, ch) { return ch.toUpperCase(); });
  }

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

  // The bytes of a text.
  //
  // The binary of a program is a list of numbers, so that no class of the
  // host is needed and the shape of the language stays the same.
  function utf8Encode(text) {
    var source = String(text === undefined || text === null ? "" : text);
    var out = [];
    for (var i = 0; i < source.length; i++) {
      var code = source.charCodeAt(i);
      // A pair of surrogates is one character of four bytes.
      if (code >= 0xd800 && code <= 0xdbff && i + 1 < source.length) {
        var next = source.charCodeAt(i + 1);
        if (next >= 0xdc00 && next <= 0xdfff) {
          code = 0x10000 + ((code - 0xd800) << 10) + (next - 0xdc00);
          i += 1;
        }
      }
      if (code < 0x80) {
        out.push(code);
      } else if (code < 0x800) {
        out.push(0xc0 | (code >> 6));
        out.push(0x80 | (code & 0x3f));
      } else if (code < 0x10000) {
        out.push(0xe0 | (code >> 12));
        out.push(0x80 | ((code >> 6) & 0x3f));
        out.push(0x80 | (code & 0x3f));
      } else {
        out.push(0xf0 | (code >> 18));
        out.push(0x80 | ((code >> 12) & 0x3f));
        out.push(0x80 | ((code >> 6) & 0x3f));
        out.push(0x80 | (code & 0x3f));
      }
    }
    return out;
  }

  function utf8Decode(bytes) {
    var list = toBytes(bytes);
    var out = "";
    var i = 0;
    while (i < list.length) {
      var byte = list[i] & 0xff;
      var code = 0;
      var size = 1;
      if (byte < 0x80) {
        code = byte;
      } else if (byte < 0xc0) {
        code = 0xfffd;
      } else if (byte < 0xe0) {
        code = ((byte & 0x1f) << 6) | ((list[i + 1] || 0) & 0x3f);
        size = 2;
      } else if (byte < 0xf0) {
        code = ((byte & 0x0f) << 12) |
          (((list[i + 1] || 0) & 0x3f) << 6) |
          ((list[i + 2] || 0) & 0x3f);
        size = 3;
      } else {
        code = ((byte & 0x07) << 18) |
          (((list[i + 1] || 0) & 0x3f) << 12) |
          (((list[i + 2] || 0) & 0x3f) << 6) |
          ((list[i + 3] || 0) & 0x3f);
        size = 4;
      }
      i += size;
      if (code > 0xffff) {
        code -= 0x10000;
        out += String.fromCharCode(0xd800 + (code >> 10), 0xdc00 + (code & 0x3ff));
      } else {
        out += String.fromCharCode(code);
      }
    }
    return out;
  }

  // The bytes of a value: a list, a buffer of Node or a text.
  function toBytes(value) {
    if (value === null || value === undefined) return [];
    if (Array.isArray(value)) return value;
    if (typeof value === "string") return utf8Encode(value);
    if (typeof value.length === "number") {
      var out = [];
      for (var i = 0; i < value.length; i++) out.push(value[i] & 0xff);
      return out;
    }
    return [];
  }

  function isBuffer(value) {
    return typeof Buffer !== "undefined" && !!Buffer.isBuffer && Buffer.isBuffer(value);
  }

  // The name of a header as it is written in the headers of an answer.
  function headerKey(headers, name) {
    var lower = String(name).toLowerCase();
    for (var key in headers) {
      if (Object.prototype.hasOwnProperty.call(headers, key) && key.toLowerCase() === lower) {
        return key;
      }
    }
    return null;
  }

  function hasHeader(headers, name) {
    return headerKey(headers, name) !== null;
  }

  function copyHeaders(target, source) {
    for (var key in source) {
      if (!Object.prototype.hasOwnProperty.call(source, key)) continue;
      var existing = headerKey(target, key);
      if (existing !== null && existing !== key) delete target[existing];
      target[key] = source[key];
    }
  }

  // The answer of the server.
  //
  // The middle pieces and the routes answer in the same way, so the body of
  // a handler becomes bytes on the wire here.
  function writeAnswer(response, result, fallbackStatus, extraHeaders, head) {
    var status = fallbackStatus === undefined ? 200 : fallbackStatus;
    var headers = {};
    var body = result;
    if (extraHeaders) copyHeaders(headers, extraHeaders);
    if (result && result.__veyResponse) {
      status = result.status;
      copyHeaders(headers, result.headers || {});
      body = result.body;
    }
    if (body === undefined || body === null) body = "";
    if (typeof body === "string") {
      if (!hasHeader(headers, "content-type")) {
        headers["Content-Type"] = "text/html; charset=utf-8";
      }
    } else if (isBuffer(body)) {
      if (!hasHeader(headers, "content-type")) {
        headers["Content-Type"] = "application/octet-stream";
      }
    } else {
      if (!hasHeader(headers, "content-type")) {
        headers["Content-Type"] = "application/json; charset=utf-8";
      }
      body = JSON.stringify(body);
    }
    if (head) {
      // The answer of a HEAD has the headers of the page, but no body.
      var size = 0;
      if (typeof body === "string") {
        size = typeof Buffer !== "undefined" && Buffer.byteLength
          ? Buffer.byteLength(body)
          : body.length;
      } else if (isBuffer(body)) {
        size = body.length;
      }
      headers["Content-Length"] = String(size);
      response.writeHead(status, headers);
      response.end();
      return;
    }
    response.writeHead(status, headers);
    response.end(body);
  }

  // The answer of a request, as the middle pieces and the routes see it.
  function makeResponse(response) {
    var state = { status: 200, headers: {}, finished: false, finish: [], transform: null, head: false };

    function answer(body) {
      if (state.finished) return;
      state.finished = true;
      if (state.transform) {
        // The body of the answer is written in another shape, such as the
        // compressed form of it.
        try {
          body = state.transform(body);
        } catch (err) {
          // A body that cannot be written in another shape is left as it is.
        }
      }
      writeAnswer(response, body, state.status, state.headers, state.head);
      for (var i = 0; i < state.finish.length; i++) {
        try {
          state.finish[i]();
        } catch (err) {
          // A mistake in a cleanup must not take the answer away.
        }
      }
    }

    var res = {
      status: function (code) {
        state.status = Number(code);
        return res;
      },
      set: function (name, value) {
        var key = headerKey(state.headers, name);
        if (key !== null && key !== String(name)) delete state.headers[key];
        state.headers[String(name)] = value;
        return res;
      },
      get: function (name) {
        var key = headerKey(state.headers, name);
        return key === null ? null : state.headers[key];
      },
      remove: function (name) {
        var key = headerKey(state.headers, name);
        if (key !== null) delete state.headers[key];
        return res;
      },
      send: function (body) {
        answer(body);
        return res;
      },
      json: function (value) {
        if (!hasHeader(state.headers, "content-type")) {
          state.headers["Content-Type"] = "application/json; charset=utf-8";
        }
        answer(JSON.stringify(value));
        return res;
      },
      end: function () {
        answer("");
        return res;
      },
      // A cookie of the answer.
      cookie: function (name, value, options) {
        var text = cookieHeader(name, value, options);
        var key = headerKey(state.headers, "set-cookie");
        var list = [];
        if (key !== null) {
          var current = state.headers[key];
          list = Array.isArray(current) ? current.slice() : [current];
        }
        list.push(text);
        state.headers[key === null ? "Set-Cookie" : key] = list;
        return res;
      },
      onFinish: function (fn) {
        state.finish.push(fn);
        return res;
      },
      // The body of the answer is written in another shape.
      transform: function (fn) {
        state.transform = fn;
        return res;
      },
      // The answer of a HEAD has the marks of the page, but no body.
      head: function (value) {
        state.head = value !== false;
        return res;
      },
      // The number of the answer.
      getStatus: function () {
        return state.status;
      },
      isFinished: function () {
        return state.finished;
      }
    };
    return res;
  }

  // A cookie of an answer, written as the header of the answer.
  function cookieHeader(name, value, options) {
    var opts = options || {};
    var text = encodeURIComponent(String(name)) + "=" + encodeURIComponent(String(value));
    text += "; Path=" + (opts.path === undefined ? "/" : opts.path);
    if (opts.domain) text += "; Domain=" + opts.domain;
    if (opts.maxAge !== undefined) text += "; Max-Age=" + opts.maxAge;
    if (opts.expires) text += "; Expires=" + opts.expires;
    if (opts.httpOnly !== false) text += "; HttpOnly";
    if (opts.secure) text += "; Secure";
    if (opts.sameSite) text += "; SameSite=" + opts.sameSite;
    return text;
  }

  // The parameters of a path, when the pattern of a route fits it.
  //
  // A part of the pattern may be a parameter, such as "{id}", which may be
  // left out, such as "{id?}", and may hold a constraint, such as
  // "{id:int}".
  var CONSTRAINTS = {
    int: /^-?\d+$/,
    number: /^-?\d+(\.\d+)?$/,
    uuid: /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/,
    slug: /^[a-zA-Z0-9_-]+$/,
    word: /^[a-zA-Z]+$/
  };

  function matchesConstraint(value, constraint) {
    if (CONSTRAINTS[constraint]) return CONSTRAINTS[constraint].test(value);
    // A constraint that is not a name of a kind is a pattern of its own.
    try {
      return new RegExp("^(?:" + constraint + ")$").test(value);
    } catch (err) {
      return true;
    }
  }

  function matchRoute(pattern, pathname) {
    var patternParts = String(pattern).split("/").filter(Boolean);
    var pathParts = String(pathname).split("/").filter(Boolean);
    var params = {};
    var i = 0;
    var j = 0;
    while (i < patternParts.length) {
      var part = patternParts[i];
      if (part === "*") {
        // A wildcard of "/*" only matches when there is something after the
        // slash, so that it does not swallow the root path.
        if (patternParts.length === 1 && pathParts.length === 0) return null;
        params.splat = safeDecode(pathParts.slice(j).join("/"));
        return params;
      }
      if (part.charAt(0) === "{" && part.charAt(part.length - 1) === "}") {
        var name = part.slice(1, -1);
        var constraint = null;
        var colon = name.indexOf(":");
        if (colon !== -1) {
          constraint = name.slice(colon + 1);
          name = name.slice(0, colon);
        }
        var optional = false;
        if (name.charAt(name.length - 1) === "?") {
          optional = true;
          name = name.slice(0, -1);
        }
        if (j >= pathParts.length) {
          if (!optional) return null;
          i += 1;
          continue;
        }
        var value = safeDecode(pathParts[j]);
        if (constraint && !matchesConstraint(value, constraint)) return null;
        params[name] = value;
        i += 1;
        j += 1;
        continue;
      }
      if (j >= pathParts.length) return null;
      if (part !== pathParts[j]) return null;
      i += 1;
      j += 1;
    }
    if (j !== pathParts.length) return null;
    return params;
  }

  ctx.flatten = flatten;
  ctx.ownProps = ownProps;
  ctx.toList = toList;
  ctx.safeDecode = safeDecode;
  ctx.toText = toText;
  ctx.cssPropertyName = cssPropertyName;
  ctx.domStyleName = domStyleName;
  ctx.veyError = veyError;
  ctx.escapeHtml = escapeHtml;
  ctx.utf8Encode = utf8Encode;
  ctx.utf8Decode = utf8Decode;
  ctx.toBytes = toBytes;
  ctx.writeAnswer = writeAnswer;
  ctx.makeResponse = makeResponse;
  ctx.cookieHeader = cookieHeader;
  ctx.matchRoute = matchRoute;

  // The timers of the host, so that a part may have helpers of the same name.
  ctx.hostSetTimeout = scope.setTimeout || setTimeout;
  ctx.hostClearTimeout = scope.clearTimeout || clearTimeout;
  ctx.hostSetInterval = scope.setInterval || setInterval;
  ctx.hostClearInterval = scope.clearInterval || clearInterval;

  return {};
}
