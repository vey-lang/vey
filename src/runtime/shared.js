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
    // A signal is written as the value it has, so that a state may be
    // written in the page without its value being taken out of it.
    if (typeof value === "object" && typeof value.subscribe === "function"
        && typeof value.peek === "function") {
      return toText(value.peek());
    }
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
      if (byte < 0x80) {
        out += String.fromCharCode(byte);
        i += 1;
        continue;
      }

      var size = 0;
      var code = 0;
      var secondMin = 0x80;
      var secondMax = 0xbf;
      if (byte >= 0xc2 && byte <= 0xdf) {
        size = 2;
        code = byte & 0x1f;
      } else if (byte >= 0xe0 && byte <= 0xef) {
        size = 3;
        code = byte & 0x0f;
        if (byte === 0xe0) secondMin = 0xa0;
        if (byte === 0xed) secondMax = 0x9f;
      } else if (byte >= 0xf0 && byte <= 0xf4) {
        size = 4;
        code = byte & 0x07;
        if (byte === 0xf0) secondMin = 0x90;
        if (byte === 0xf4) secondMax = 0x8f;
      } else {
        out += "\ufffd";
        i += 1;
        continue;
      }

      var consumed = 1;
      var valid = true;
      while (consumed < size) {
        if (i + consumed >= list.length) {
          valid = false;
          break;
        }
        var next = list[i + consumed] & 0xff;
        var minimum = consumed === 1 ? secondMin : 0x80;
        var maximum = consumed === 1 ? secondMax : 0xbf;
        if (next < minimum || next > maximum) {
          valid = false;
          break;
        }
        code = (code << 6) | (next & 0x3f);
        consumed += 1;
      }
      if (!valid) {
        out += "\ufffd";
        // Consume the valid prefix, but leave a non-continuation byte to be
        // decoded on its own. This preserves valid text after a bad sequence.
        i += consumed;
        continue;
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
      if (!hasHeader(headers, "content-length")) {
        headers["Content-Length"] = String(size);
      }
      response.writeHead(status, headers);
      response.end();
      return;
    }
    response.writeHead(status, headers);
    response.end(body);
  }

  // The answer of a request, as the middle pieces and the routes see it.
  function makeResponse(response) {
    var state = {
      status: 200,
      headers: {},
      finished: false,
      finishDone: false,
      finish: [],
      // Whether the answer is written in parts.
      streaming: false,
      // Whether the head of the answer is already written.
      sent: false,
      // The work that runs just before the body is written, so that a mark
      // of the answer may still be added.
      before: [],
      beforeDone: false,
      transform: null,
      head: false
    };

    function runBeforeFinish() {
      if (state.beforeDone) return;
      state.beforeDone = true;
      for (var j = 0; j < state.before.length; j++) {
        try {
          state.before[j]();
        } catch (err) {
          // A mistake in a mark must not take the answer away.
        }
      }
    }

    function runFinish() {
      if (state.finishDone) return;
      state.finishDone = true;
      for (var i = 0; i < state.finish.length; i++) {
        try {
          state.finish[i]();
        } catch (err) {
          // A mistake in a cleanup must not take the answer away.
        }
      }
    }

    if (response && typeof response.on === "function") {
      response.on("close", function () {
        if (state.finished) return;
        state.finished = true;
        runFinish();
      });
    }

    function answer(body) {
      if (state.finished) return;
      state.finished = true;
      // The parts of the answer that were written are the answer, so it
      // is only ended.
      if (state.streaming) {
        runBeforeFinish();
        if (!state.sent) {
          state.sent = true;
          response.writeHead(state.status, state.headers);
        }
        response.end();
        runFinish();
        return;
      }
      // The marks of the answer are added before the body is written, so
      // that they are not lost when the answer ends by itself.
      runBeforeFinish();
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
      runFinish();
    }

    var res = {
      status: function (code) {
        state.status = Number(code);
        return res;
      },
      set: function (name, value) {
        var key = headerKey(state.headers, name);
        if (key !== null) {
          var current = state.headers[key];
          // A mark that may be written more than once, such as the cookie
          // of the answer, keeps the ones that are already there.
          if (Array.isArray(current)) {
            current.push(value);
            return res;
          }
          if (key !== String(name)) {
            delete state.headers[key];
            state.headers[String(name)] = value;
            return res;
          }
        }
        state.headers[String(name)] = value;
        return res;
      },
      get: function (name) {
        var key = headerKey(state.headers, name);
        return key === null ? null : state.headers[key];
      },
      // The marks of the answer, as they are written now.
      getHeaders: function () {
        return state.headers;
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
      // Another address is shown.
      redirect: function (location, status) {
        state.status = status === undefined ? 302 : Number(status);
        state.headers["Location"] = String(location);
        answer("");
        return res;
      },
      // The answer is only the number of it.
      sendStatus: function (code) {
        state.status = Number(code);
        answer("");
        return res;
      },
      // The kind of the body of the answer.
      type: function (kind) {
        state.headers["Content-Type"] = String(kind);
        return res;
      },
      contentType: function (kind) {
        state.headers["Content-Type"] = String(kind);
        return res;
      },
      // The answer depends on the head of the request.
      vary: function (name) {
        var key = headerKey(state.headers, "vary");
        var current = key === null ? null : state.headers[key];
        if (current === null || current === "") {
          state.headers["Vary"] = String(name);
        } else if (String(current).indexOf(String(name)) === -1) {
          state.headers[key] = String(current) + ", " + String(name);
        }
        return res;
      },
      // Where the answer points.
      location: function (url) {
        state.headers["Location"] = String(url);
        return res;
      },
      // A part of the answer is written, and the answer is not finished.
      //
      // The parts are written as they come, so that a big answer does
      // not have to be in the memory of the server.
      write: function (text) {
        if (state.finished) return res;
        state.streaming = true;
        runBeforeFinish();
        if (!state.sent) {
          state.sent = true;
          response.writeHead(state.status, state.headers);
        }
        response.write(isBuffer(text) ? text : String(text));
        return res;
      },
      // Write the status line and headers without ending the answer.
      writeHead: function (code, headers) {
        if (state.finished || state.sent) return res;
        state.status = Number(code);
        if (headers) copyHeaders(state.headers, headers);
        runBeforeFinish();
        state.streaming = true;
        state.sent = true;
        response.writeHead(state.status, state.headers);
        return res;
      },
      // Send headers now, leaving the body for a later write or end call.
      flushHeaders: function () {
        if (state.finished || state.sent) return res;
        runBeforeFinish();
        state.sent = true;
        if (typeof response.flushHeaders === "function") {
          response.writeHead(state.status, state.headers);
          response.flushHeaders();
        }
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
      // The work that runs just before the body is written.
      //
      // A mark of the answer that is only known when the answer ends, such
      // as the cookie of a session, is written here.
      beforeFinish: function (fn) {
        state.before.push(fn);
        return res;
      },
      // The body of the answer is written in another shape.
      //
      // Two pieces may want to change the body, so the changes are made one
      // after the other and none of them is lost.
      transform: function (fn) {
        var before = state.transform;
        if (!before) {
          state.transform = fn;
        } else {
          state.transform = function (body) {
            return fn(before(body));
          };
        }
        return res;
      },
      // A mark of the answer is added to the one that is already there.
      //
      // The marks that may be written more than once, such as `Vary`, are
      // joined with a comma.
      append: function (name, value) {
        var key = headerKey(state.headers, name);
        var current = key === null ? null : state.headers[key];
        if (current === null || current === undefined || current === "") {
          return res.set(name, value);
        }
        if (Array.isArray(current)) {
          current.push(String(value));
          return res;
        }
        state.headers[key] = String(current) + ", " + String(value);
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
    res.locals = {};
    Object.defineProperty(res, "headersSent", {
      enumerable: true,
      get: function () { return state.sent; }
    });
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
    // Optional parts try to consume first, preserving the existing greedy
    // behavior when the rest of the route still fits. Failed suffixes are
    // remembered so ambiguous routes do not repeatedly search the same path.
    var failed = Object.create(null);

    function matchFrom(i, j, params) {
      var key = i + ":" + j;
      if (failed[key]) return null;
      if (i === patternParts.length) {
        if (j === pathParts.length) return params;
        failed[key] = true;
        return null;
      }

      var part = patternParts[i];
      if (part === "*") {
        // A wildcard of "/*" only matches when there is something after the
        // slash, so that it does not swallow the root path.
        if (patternParts.length === 1 && pathParts.length === 0) {
          failed[key] = true;
          return null;
        }
        var wildcardParams = Object.assign({}, params);
        wildcardParams.splat = safeDecode(pathParts.slice(j).join("/"));
        return wildcardParams;
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

        if (j < pathParts.length) {
          var value = safeDecode(pathParts[j]);
          if (!constraint || matchesConstraint(value, constraint)) {
            var consumedParams = Object.assign({}, params);
            consumedParams[name] = value;
            var consumed = matchFrom(i + 1, j + 1, consumedParams);
            if (consumed) return consumed;
          }
        }

        if (optional) {
          var skipped = matchFrom(i + 1, j, params);
          if (skipped) return skipped;
        }
        failed[key] = true;
        return null;
      }

      if (j < pathParts.length && part === pathParts[j]) {
        var matched = matchFrom(i + 1, j + 1, params);
        if (matched) return matched;
      }
      failed[key] = true;
      return null;
    }

    return matchFrom(0, 0, {});
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
