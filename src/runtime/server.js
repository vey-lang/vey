// The server of a program.
function veyServer(ctx) {
  var query = ctx.query;
  var print = ctx.print;
  var log = ctx.log;
  var req = ctx.req;
  var safeDecode = ctx.safeDecode;
  var veyError = ctx.veyError;

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
          } else if (contentType.indexOf("application/x-www-form-urlencoded") !== -1) {
            // The body of a form is a query.
            parsed = query.parse(raw);
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
      ctx.server = server;
  ctx.response = response;
  ctx.redirect = redirect;

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
    server: server,
    response: response,
    redirect: redirect,
  };
}
