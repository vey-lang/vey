// The server of a program.
function veyServer(ctx) {
  var query = ctx.query;
  var print = ctx.print;
  var log = ctx.log;
  var req = ctx.req;
  var safeDecode = ctx.safeDecode;
  var veyError = ctx.veyError;
  var makeResponse = ctx.makeResponse;
  var sessions = ctx.sessions;
  var matchRoute = ctx.matchRoute;
  var hostSetTimeout = ctx.hostSetTimeout;

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

  // How long a file that is served is kept by the browser.
  function cacheControlOf(ext) {
    if (ext === ".html") return "no-cache";
    if (
      ext === ".svg" || ext === ".png" || ext === ".jpg" || ext === ".jpeg" ||
      ext === ".gif" || ext === ".webp" || ext === ".ico" || ext === ".woff2"
    ) {
      return "public, max-age=31536000, immutable";
    }
    return "public, max-age=3600";
  }

  // The fields and the files of a form of the kind "multipart/form-data".
  function parseMultipart(buffer, contentType) {
    var found = /boundary=(?:"([^"]+)"|([^;]+))/i.exec(contentType);
    if (!found) return null;
    var mark = String(found[1] || found[2]).trim();
    var dash = Buffer.from("--" + mark);
    var step = Buffer.from("\r\n--" + mark);
    var fields = {};
    var files = {};
    var position = 0;

    for (;;) {
      // Every part begins with the name of the boundary.
      var start = buffer.indexOf(dash, position);
      if (start === -1) break;
      // The list of the parts ends with "--boundary--".
      var after = start + dash.length;
      if (buffer.slice(after, after + 2).toString() === "--") break;
      // The marks of the part begin after the line of the boundary.
      var headersStart = after;
      if (buffer.slice(headersStart, headersStart + 2).toString() === "\r\n") {
        headersStart += 2;
      } else {
        break;
      }
      var headerEnd = buffer.indexOf("\r\n\r\n", headersStart);
      if (headerEnd === -1) break;
      var headerText = buffer.slice(headersStart, headerEnd).toString("utf8");
      var contentStart = headerEnd + 4;

      // The content of the part ends before the next boundary.
      var next = buffer.indexOf(step, contentStart);
      if (next === -1) break;
      var content = buffer.slice(contentStart, next);

      var name = null;
      var filename = null;
      var kind = "text/plain";
      headerText.split("\r\n").forEach(function (line) {
        var index = line.indexOf(":");
        if (index === -1) return;
        var key = line.slice(0, index).trim().toLowerCase();
        var value = line.slice(index + 1).trim();
        if (key === "content-disposition") {
          var named = /name="([^"]*)"/i.exec(value) || /name=([^;]+)/i.exec(value);
          if (named) name = String(named[1]).trim();
          var file = /filename="([^"]*)"/i.exec(value);
          if (file) filename = file[1];
        } else if (key === "content-type") {
          kind = value;
        }
      });

      if (name !== null) {
        if (filename !== null) {
          var entry = {
            filename: filename,
            contentType: kind,
            size: content.length,
            bytes: content,
            text: content.toString("utf8"),
            isFile: true
          };
          files[name] = entry;
          fields[name] = entry;
        } else {
          fields[name] = content.toString("utf8");
        }
      }
      position = next + 2;
    }
    return { fields: fields, files: files };
  }

  function server(port, options) {
    if (!req) throw veyError("server() is only available in Node.");
    var httpModule = req("node:http");
    var fs = req("node:fs");
    var pathModule = req("node:path");
    var cryptoModule = req("node:crypto");
    var opts = options || {};
    var routes = [];
    var middlewares = [];
    var sockets = [];
    var streams = [];
    var errorHandler = null;
    var staticDir = null;
    var notFoundHandler = null;
    var actualPort = port === undefined ? 3000 : port;
    // The body of a request is read into the memory, so it has a limit.
    var limit = opts.bodyLimit === undefined ? 10 * 1024 * 1024 : Number(opts.bodyLimit);
    // The requests that are running now.
    var running = 0;

    function add(method, pattern, handler, extra) {
      routes.push({
        method: method,
        pattern: pattern,
        handler: handler,
        middlewares: extra || []
      });
    }

    // The route of a path, when there is one.
    //
    // The routes are looked at in the order they were written, so that the
    // first one that fits is the one that answers.
    function findRoute(methods, pathname) {
      for (var i = 0; i < routes.length; i++) {
        var route = routes[i];
        if (methods.indexOf(route.method) === -1) continue;
        var params = matchRoute(route.pattern, pathname);
        if (params) return { route: route, params: params };
      }
      return null;
    }

    // The methods of a path, for the answer of an OPTIONS.
    function allowedMethods(pathname) {
      var found = {};
      for (var i = 0; i < routes.length; i++) {
        var route = routes[i];
        if (!matchRoute(route.pattern, pathname)) continue;
        found[route.method] = true;
      }
      found.OPTIONS = true;
      if (found.GET) found.HEAD = true;
      var out = [];
      for (var method in found) out.push(method);
      return out.join(", ");
    }

    // Whether a middle piece of a path fits another path.
    function fitsPath(pattern, pathname) {
      if (!pattern) return true;
      if (pattern === "/") return true;
      if (pathname === pattern) return true;
      return pathname.indexOf(pattern + "/") === 0;
    }

    function readBody(request) {
      return new Promise(function (resolve, reject) {
        var chunks = [];
        var size = 0;
        var tooLarge = false;
        request.on("data", function (chunk) {
          size += chunk.length;
          if (size > limit) {
            tooLarge = true;
            return;
          }
          chunks.push(chunk);
        });
        request.on("end", function () {
          if (tooLarge) {
            var error = veyError("the request is too large");
            error.tooLarge = true;
            reject(error);
            return;
          }
          var buffer = Buffer.concat(chunks);
          var raw = buffer.toString("utf8");
          var parsed = raw;
          var files = {};
          var contentType = request.headers["content-type"] || "";
          if (contentType.indexOf("json") !== -1 && raw.trim()) {
            try { parsed = JSON.parse(raw); } catch (err) { parsed = raw; }
          } else if (contentType.indexOf("multipart/form-data") !== -1) {
            // A form with files, which the visitor sends.
            var form = parseMultipart(buffer, contentType);
            if (form) {
              parsed = form.fields;
              files = form.files;
            }
          } else if (contentType.indexOf("application/x-www-form-urlencoded") !== -1) {
            // The body of a form is a query.
            parsed = query.parse(raw);
          }
          resolve({ raw: raw, parsed: parsed, files: files });
        });
        request.on("error", reject);
      });
    }

    // The file of a folder is served, with the marks of the cache.
    function serveStatic(request, res, pathname, status) {
      var relative = safeDecode(pathname).replace(/^\/+/, "");
      var file = pathModule.resolve(staticDir, relative);
      if (file !== staticDir && file.indexOf(staticDir + pathModule.sep) !== 0) {
        res.status(403);
        res.send("Forbidden");
        return true;
      }
      try {
        var stat = fs.statSync(file);
        if (stat.isDirectory()) file = pathModule.join(file, "index.html");
        if (!fs.existsSync(file)) return false;
        var info = fs.statSync(file);
        var ext = pathModule.extname(file).toLowerCase();
        var etag = '"' + info.size.toString(16) + "-" + info.mtimeMs.toString(16) + '"';
        res.set("Content-Type", MIME[ext] || "application/octet-stream");
        res.set("ETag", etag);
        res.set("Last-Modified", new Date(info.mtime).toUTCString());
        res.set("Cache-Control", cacheControlOf(ext));
        // The file is not sent again when the visitor has it already.
        var match = request.headers["if-none-match"];
        if (match && match === etag) {
          res.status(304);
          res.end();
          return true;
        }
        var since = request.headers["if-modified-since"];
        if (since && new Date(since).getTime() >= Math.floor(info.mtime.getTime() / 1000) * 1000) {
          res.status(304);
          res.end();
          return true;
        }
        res.status(status === undefined ? 200 : status);
        if (request.method === "HEAD") {
          res.set("Content-Length", String(info.size));
          res.end();
          return true;
        }
        res.send(fs.readFileSync(file));
        return true;
      } catch (err) {
        return false;
      }
    }

    // The address of the visitor.
    function ipOf(request) {
      var forwarded = request.headers["x-forwarded-for"];
      if (forwarded) return String(forwarded).split(",")[0].trim();
      if (request.socket && request.socket.remoteAddress) return request.socket.remoteAddress;
      return "";
    }

    function protocolOf(request) {
      var forwarded = request.headers["x-forwarded-proto"];
      if (forwarded) return String(forwarded).split(",")[0].trim();
      if (request.socket && request.socket.encrypted) return "https";
      return "http";
    }

    function makeRequest(request, url, params) {
      var query_ = {};
      url.searchParams.forEach(function (value, key) { query_[key] = value; });
      var headers = request.headers || {};
      return {
        path: url.pathname,
        method: request.method,
        params: params,
        query: query_,
        headers: headers,
        body: null,
        text: "",
        files: {},
        cookies: parseCookies(headers.cookie),
        // The value of a mark of the request.
        get: function (name) {
          var value = headers[String(name).toLowerCase()];
          return value === undefined ? null : value;
        },
        ip: ipOf(request),
        host: headers.host || "",
        protocol: protocolOf(request)
      };
    }

    // A mistake is answered.
    function fail(info, res, err) {
      if (err && err.tooLarge) {
        res.status(413);
        res.send("The request is too large.");
        return;
      }
      if (errorHandler) {
        try {
          errorHandler(err, info, res);
          return;
        } catch (ignored) {
          // The handler of the mistakes had a mistake of its own.
        }
      }
      log.error(err);
      res.status(500);
      res.send("Server error: " + (err && err.message ? err.message : err));
    }

    // The middle pieces are run, one after the other.
    function runChain(list, index, info, res, done) {
      if (index >= list.length) {
        done();
        return;
      }
      var piece = list[index];
      var called = false;
      function next(err) {
        if (called) return;
        called = true;
        if (err) {
          fail(info, res, err);
          return;
        }
        runChain(list, index + 1, info, res, done);
      }
      try {
        piece(info, res, next);
      } catch (err) {
        if (called) {
          // The piece has already gone on, so the mistake is answered here.
          fail(info, res, err);
          return;
        }
        next(err);
      }
    }

    // The answer of a request that no route fits.
    function notFound(request, info, res) {
      if (staticDir && serveStatic(request, res, info.path)) return;
      if (staticDir && serveStatic(request, res, "/404.html", 404)) return;
      if (notFoundHandler) {
        Promise.resolve(notFoundHandler(info)).then(function (result) {
          if (res.isFinished()) return;
          res.status(404);
          res.send(result);
        }).catch(function (err) {
          fail(info, res, err);
        });
        return;
      }
      res.status(404);
      res.send("Not found");
    }

    // The socket of a visitor, which stays open.
    function makeSocket(socket) {
      var buffer = Buffer.alloc(0);
      var messageHandlers = [];
      var closeHandlers = [];
      var open = true;

      function frame(opcode, data) {
        var header;
        if (data.length < 126) {
          header = Buffer.from([opcode, data.length]);
        } else if (data.length < 65536) {
          header = Buffer.alloc(4);
          header[0] = opcode;
          header[1] = 126;
          header.writeUInt16BE(data.length, 2);
        } else {
          header = Buffer.alloc(10);
          header[0] = opcode;
          header[1] = 127;
          header.writeUInt32BE(Math.floor(data.length / 4294967296), 2);
          header.writeUInt32BE(data.length % 4294967296, 6);
        }
        socket.write(Buffer.concat([header, data]));
      }

      function send(text) {
        if (!open) return;
        frame(0x81, Buffer.from(String(text), "utf8"));
      }

      function close() {
        if (!open) return;
        open = false;
        try { frame(0x88, Buffer.alloc(0)); } catch (err) { }
        try { socket.end(); } catch (err) { }
        for (var i = 0; i < closeHandlers.length; i++) closeHandlers[i]();
      }

      function read(chunk) {
        buffer = Buffer.concat([buffer, chunk]);
        for (;;) {
          if (buffer.length < 2) return;
          var first = buffer[0];
          var second = buffer[1];
          var opcode = first & 0x0f;
          var masked = (second & 0x80) !== 0;
          var length = second & 0x7f;
          var offset = 2;
          if (length === 126) {
            if (buffer.length < offset + 2) return;
            length = buffer.readUInt16BE(offset);
            offset += 2;
          } else if (length === 127) {
            if (buffer.length < offset + 8) return;
            length = buffer.readUInt32BE(offset + 4);
            offset += 8;
          }
          var mask = null;
          if (masked) {
            if (buffer.length < offset + 4) return;
            mask = buffer.slice(offset, offset + 4);
            offset += 4;
          }
          if (buffer.length < offset + length) return;
          var payload = buffer.slice(offset, offset + length);
          if (mask) {
            for (var i = 0; i < payload.length; i++) payload[i] = payload[i] ^ mask[i % 4];
          }
          buffer = buffer.slice(offset + length);
          if (opcode === 0x8) {
            close();
            return;
          }
          if (opcode === 0x9) {
            // The answer of a ping is a pong.
            if (open) frame(0x8a, payload);
            continue;
          }
          if (opcode === 0x1 || opcode === 0x2 || opcode === 0x0) {
            var text = payload.toString("utf8");
            for (var j = 0; j < messageHandlers.length; j++) messageHandlers[j](text);
          }
        }
      }

      socket.on("data", function (chunk) { read(chunk); });
      socket.on("close", function () { close(); });
      socket.on("error", function () { close(); });

      return {
        send: send,
        close: close,
        onMessage: function (fn) {
          messageHandlers.push(fn);
          return this;
        },
        onClose: function (fn) {
          closeHandlers.push(fn);
          return this;
        }
      };
    }

    // The events of the server, as they come.
    function makeStream(response, onClose, extra) {
      var open = true;
      var headers = {};
      for (var key in extra || {}) headers[key] = extra[key];
      headers["Content-Type"] = "text/event-stream; charset=utf-8";
      headers["Cache-Control"] = "no-cache";
      headers["Connection"] = "keep-alive";
      headers["X-Accel-Buffering"] = "no";
      response.writeHead(200, headers);
      return {
        send: function (data) {
          if (!open) return;
          response.write("data: " + String(data) + "\n\n");
        },
        event: function (name, data) {
          if (!open) return;
          response.write("event: " + String(name) + "\n");
          response.write("data: " + String(data) + "\n\n");
        },
        close: function () {
          if (!open) return;
          open = false;
          response.end();
          if (onClose) onClose();
        }
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

    // A socket of a visitor asks for another kind of answer.
    serverInstance.on("upgrade", function (request, socket, head) {
      try {
        var url = new URL(request.url || "/", "http://" + (request.headers.host || "localhost"));
        var found = findRoute(["WS"], url.pathname);
        if (!found) {
          socket.destroy();
          return;
        }
        var key = request.headers["sec-websocket-key"];
        if (!key) {
          socket.destroy();
          return;
        }
        var accept = cryptoModule
          .createHash("sha1")
          .update(key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11")
          .digest("base64");
        socket.write(
          "HTTP/1.1 101 Switching Protocols\r\n" +
            "Upgrade: websocket\r\n" +
            "Connection: Upgrade\r\n" +
            "Sec-WebSocket-Accept: " + accept + "\r\n\r\n"
        );
        var connection = makeSocket(socket);
        sockets.push(connection);
        found.route.handler(connection, makeRequest(request, url, found.params));
      } catch (err) {
        log.error(err);
        try { socket.destroy(); } catch (ignored) { }
      }
    });

    function handleRequest(request, response) {
      var url = new URL(request.url || "/", "http://" + (request.headers.host || "localhost"));
      var res = makeResponse(response);
      var method = request.method;

      // A HEAD is the same as a GET, without the body of the answer.
      // The events of the server are asked for with a GET as well.
      var methods = [method];
      if (method === "HEAD") methods = ["GET"];
      if (method === "GET") methods = ["GET", "SSE"];
      var found = findRoute(methods, url.pathname);
      res.head(method === "HEAD");

      // The middle pieces of the server, and the ones of the route.
      var pieces = [];
      for (var i = 0; i < middlewares.length; i++) {
        if (fitsPath(middlewares[i].path, url.pathname)) pieces.push(middlewares[i].fn);
      }
      if (found) {
        for (var j = 0; j < found.route.middlewares.length; j++) {
          pieces.push(found.route.middlewares[j]);
        }
      }

      running += 1;
      res.onFinish(function () { running -= 1; });
      readBody(request).then(function (body) {
        var info = makeRequest(request, url, found ? found.params : {});
        info.body = body.parsed;
        info.text = body.raw;
        info.files = body.files;
        var store = sessions.open(info.cookies);
        sessions.run(store, function () {
          runChain(pieces, 0, info, res, function () {
            if (res.isFinished()) return;
            // The question of the methods of an address is answered.
            if (method === "OPTIONS") {
              res.set("Allow", allowedMethods(url.pathname));
              res.status(204);
              res.end();
              return;
            }
            if (!found) {
              notFound(request, info, res);
              return;
            }
            // The events of the server keep the answer open.
            if (found.route.method === "SSE") {
              var stream = makeStream(response, function () { running -= 1; }, res.getHeaders());
              streams.push(stream);
              Promise.resolve(found.route.handler(info, stream)).catch(function (err) {
                fail(info, res, err);
              });
              return;
            }
            Promise.resolve(found.route.handler(info, res)).then(function (result) {
              if (res.isFinished()) return;
              var header = sessions.header(store);
              if (header) res.set("Set-Cookie", header);
              res.send(result);
            }).catch(function (err) {
              fail(info, res, err);
            });
          });
        });
      }).catch(function (err) {
        var info = makeRequest(request, url, {});
        fail(info, res, err);
      });
    }

    // The methods of the server are the same for every path.
    function methodOf(name) {
      return function (pattern, handler) {
        var extra = [];
        for (var i = 2; i < arguments.length; i++) extra.push(arguments[i]);
        add(name, pattern, handler, extra);
        return this;
      };
    }

    return {
      get: methodOf("GET"),
      post: methodOf("POST"),
      put: methodOf("PUT"),
      patch: methodOf("PATCH"),
      del: methodOf("DELETE"),
      route: function (method, pattern, handler) {
        var extra = [];
        for (var i = 3; i < arguments.length; i++) extra.push(arguments[i]);
        add(String(method).toUpperCase(), pattern, handler, extra);
        return this;
      },
      // A socket of a visitor, which stays open.
      ws: function (pattern, handler) {
        add("WS", pattern, handler, []);
        return this;
      },
      // The events of the server, as they come.
      sse: function (pattern, handler) {
        add("SSE", pattern, handler, []);
        return this;
      },
      // A middle piece of the server.
      use: function (path, fn) {
        if (typeof path === "function") {
          middlewares.push({ path: null, fn: path });
        } else {
          middlewares.push({ path: String(path), fn: fn });
        }
        return this;
      },
      // The piece that catches the mistakes.
      error: function (handler) {
        errorHandler = handler;
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
          actualPort = port;
          // The host the server listens on, so that the line that says it
          // is listening names the right one.
          if (address && address.address) serverHost = address.address;
          print(
            "Vey server listening on http://" +
              (serverHost || "localhost") + ":" + port
          );
          if (callback) callback();
        });
        return this;
      },
      // The port the server listens on.
      port: function () {
        var address = serverInstance.address();
        return address && address.port ? address.port : actualPort;
      },
      // The server stops, and the requests that are running are waited for.
      stop: function () {
        return new Promise(function (resolve) {
          for (var i = 0; i < sockets.length; i++) sockets[i].close();
          for (var j = 0; j < streams.length; j++) streams[j].close();
          var closed = false;
          function finish() {
            if (closed) return;
            closed = true;
            resolve();
          }
          function wait() {
            if (running === 0) {
              serverInstance.close(finish);
              if (serverInstance.closeIdleConnections) serverInstance.closeIdleConnections();
              return;
            }
            hostSetTimeout(wait, 20);
          }
          wait();
        });
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

  ctx.server = server;
  ctx.response = response;
  ctx.redirect = redirect;

  return {
    server: server,
    response: response,
    redirect: redirect,
  };
}
