// The server of a program.
function veyServer(ctx) {
  var isNode = ctx.isNode;
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

  // The cookies of a request are read by the reader that the pages use, so
  // that one head gives the same answer on both sides.
  var parseCookies = ctx.cookie.parse;

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
  // The time that a file of a folder is kept in the browser.
  var staticMaxAge = 0;

  function cacheControlOf(ext) {
    if (ext === ".html") return "no-cache";
    if (
      ext === ".svg" || ext === ".png" || ext === ".jpg" || ext === ".jpeg" ||
      ext === ".gif" || ext === ".webp" || ext === ".ico" || ext === ".woff2"
    ) {
      return "public, max-age=31536000, immutable";
    }
    if (staticMaxAge) return "public, max-age=" + staticMaxAge;
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
    var middlewareErrorHandler = null;
    var errorHandlers = [];
    var staticDir = null;
    // The name of the file of a folder that is served.
    var staticIndex = "index.html";
    var notFoundHandler = null;
    var actualPort = port === undefined ? 3000 : port;
    // The body of a request is read into the memory, so it has a limit.
    var limit = opts.bodyLimit === undefined ? 10 * 1024 * 1024 : Number(opts.bodyLimit);
    // The time that a request may take, before it is refused.
    var requestTimeout = opts.timeout === undefined ? 0 : Number(opts.timeout);
    // Whether a proxy is in front of the server.
    //
    // The heads that a proxy writes are only read when the program says
    // so, because a visitor may write them as well.
    var trustProxy = opts.trustProxy === true || opts.trustProxy === "true";
    // Whether the reason of a mistake is told to the visitor.
    //
    // The reason may tell something about the machine of the server, so it
    // is only told when the program says that it may be.
    var exposeErrors = opts.exposeErrors === true || opts.exposeErrors === "true";
    // The requests that are running now.
    var running = 0;
    var shutdownCallbacks = [];
    var signalListeners = {};
    var signalsInstalled = false;
    var app = null;
    var processModule = null;
    if (isNode) {
      try { processModule = ctx.nodeModule("node:process"); } catch (ignored) { }
    }

    function installSignals() {
      if (!processModule || signalsInstalled || typeof processModule.on !== "function") return;
      ["SIGTERM", "SIGINT"].forEach(function (signal) {
        var listener = function () {
          var work = [];
          for (var i = 0; i < shutdownCallbacks.length; i++) {
            try { work.push(Promise.resolve(shutdownCallbacks[i](signal))); } catch (err) { work.push(Promise.reject(err)); }
          }
          Promise.allSettled(work).then(function () {
            if (app) app.stop();
          });
        };
        signalListeners[signal] = listener;
        processModule.on(signal, listener);
      });
      signalsInstalled = true;
    }

    function removeSignals() {
      if (!processModule || !signalsInstalled || typeof processModule.removeListener !== "function") return;
      for (var signal in signalListeners) processModule.removeListener(signal, signalListeners[signal]);
      signalListeners = {};
      signalsInstalled = false;
    }

    function add(method, pattern, handler, extra) {
      // A middle piece may ask for a smaller body. The limit of the route
      // is known here, so that the body is not read at all when it is too
      // large.
      var routeLimit = undefined;
      var list = extra || [];
      for (var i = 0; i < list.length; i++) {
        var asked = list[i] && list[i].bodyLimit;
        if (typeof asked === "number" && isFinite(asked)) {
          routeLimit = routeLimit === undefined ? asked : Math.min(routeLimit, asked);
        }
      }
      routes.push({
        method: method,
        pattern: pattern,
        handler: handler,
        middlewares: list,
        limit: routeLimit
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
      if (pathname.indexOf(pattern + "/") === 0) return true;
      // Middleware paths use the same parameter and wildcard syntax as
      // routes, while retaining the useful prefix behaviour above.
      if (matchRoute(pattern, pathname) !== null) return true;
      var patternParts = String(pattern).split("/").filter(Boolean);
      var pathParts = String(pathname).split("/").filter(Boolean);
      if (patternParts.length <= pathParts.length && patternParts.indexOf("*") === -1) {
        var prefix = "/" + pathParts.slice(0, patternParts.length).join("/");
        return matchRoute(pattern, prefix) !== null;
      }
      return false;
    }

    function readBody(request, max) {
      return new Promise(function (resolve, reject) {
        var chunks = [];
        var size = 0;
        var tooLarge = false;
        var limitValue = max === undefined ? limit : max;
        request.on("data", function (chunk) {
          if (tooLarge) return;
          size += chunk.length;
          if (size > limitValue) {
            // The body is too large, so it is not kept and the reading
            // stops while the server sends the 413 response.
            tooLarge = true;
            chunks.length = 0;
            request.pause();
            var error = veyError("the request is too large");
            error.tooLarge = true;
            reject(error);
            return;
          }
          chunks.push(chunk);
        });
        request.on("end", function () {
          if (tooLarge) return;
          var buffer = Buffer.concat(chunks);
          var raw = buffer.toString("utf8");
          var parsed = raw;
          var files = {};
          var contentType = request.headers["content-type"] || "";
          if (contentType.indexOf("json") !== -1 && raw.trim()) {
            try {
              parsed = JSON.parse(raw);
            } catch (err) {
              err.badRequest = true;
              reject(err);
              return;
            }
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

    function isWithinStaticRoot(root, file) {
      var relative = pathModule.relative(root, file);
      return relative === "" || (
        relative !== ".." &&
        relative.indexOf(".." + pathModule.sep) !== 0 &&
        !pathModule.isAbsolute(relative)
      );
    }

    // The file of a folder is served, with the marks of the cache.
    function serveStatic(request, res, pathname, status) {
      var relative = safeDecode(pathname).replace(/^\/+/, "");
      try {
        var root = fs.realpathSync(staticDir);
        var file = pathModule.resolve(root, relative);
        if (!isWithinStaticRoot(root, file)) {
          res.status(403);
          res.send("Forbidden");
          return true;
        }
        file = fs.realpathSync(file);
        if (!isWithinStaticRoot(root, file)) {
          res.status(403);
          res.send("Forbidden");
          return true;
        }
        var stat = fs.statSync(file);
        if (stat.isDirectory()) {
          file = pathModule.resolve(file, staticIndex);
          if (!isWithinStaticRoot(root, file)) {
            res.status(403);
            res.send("Forbidden");
            return true;
          }
          if (!fs.existsSync(file)) return false;
          file = fs.realpathSync(file);
          if (!isWithinStaticRoot(root, file)) {
            res.status(403);
            res.send("Forbidden");
            return true;
          }
        }
        var info = fs.statSync(file);
        var ext = pathModule.extname(file).toLowerCase();
        var etag = '"' + info.size.toString(16) + "-" + info.mtimeMs.toString(16) + '"';
        res.set("Content-Type", MIME[ext] || "application/octet-stream");
        res.set("ETag", etag);
        res.set("Last-Modified", new Date(info.mtime).toUTCString());
        res.set("Cache-Control", cacheControlOf(ext));
        res.set("Accept-Ranges", "bytes");
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
        // A part of the file may be asked for, so that a big file may be
        // played without being read whole.
        var range = request.headers["range"];
        if (range) {
          var wanted = parseRange(range, info.size);
          if (wanted === false) {
            res.status(416);
            res.set("Content-Range", "bytes */" + info.size);
            res.end();
            return true;
          }
          if (wanted) {
            res.status(206);
            res.set(
              "Content-Range",
              "bytes " + wanted.start + "-" + wanted.end + "/" + info.size
            );
            res.set("Content-Length", String(wanted.end - wanted.start + 1));
            if (request.method === "HEAD") {
              res.end();
              return true;
            }
            res.send(readPart(file, wanted.start, wanted.end));
            return true;
          }
        }
        res.status(status === undefined ? 200 : status);
        if (request.method === "HEAD") {
          res.set("Content-Length", String(info.size));
          res.end();
          return true;
        }
        res.sendFile(file);
        return true;
      } catch (err) {
        return false;
      }
    }

    // The part of a file that is asked for.
    //
    // The answer is false when the range is not one that may be given,
    // and null when there is no range.
    function parseRange(header, size) {
      var text = String(header).trim();
      var match = /^bytes=(\d*)-(\d*)$/.exec(text);
      if (!match) return false;
      var from = match[1] === "" ? null : Number(match[1]);
      var to = match[2] === "" ? null : Number(match[2]);
      if (from === null && to === null) return false;
      var start;
      var end;
      if (from === null) {
        // The last parts of the file are asked for.
        var length = to;
        start = Math.max(0, size - length);
        end = size - 1;
      } else {
        start = from;
        end = to === null ? size - 1 : Math.min(to, size - 1);
      }
      if (start > end || start >= size) return false;
      return { start: start, end: end };
    }

    // A part of a file.
    function readPart(file, start, end) {
      var length = end - start + 1;
      var buffer = Buffer.alloc(length);
      var opened = fs.openSync(file, "r");
      try {
        fs.readSync(opened, buffer, 0, length, start);
      } finally {
        fs.closeSync(opened);
      }
      return buffer;
    }

    // The address of the visitor.
    //
    // The head of a proxy in front of the server says who the visitor is,
    // but a visitor may write that head as well. It is only read when the
    // program says that a proxy is in front of it.
    function ipOf(request, trust) {
      if (trust) {
        var forwarded = request.headers["x-forwarded-for"];
        if (forwarded) return String(forwarded).split(",")[0].trim();
      }
      if (request.socket && request.socket.remoteAddress) return request.socket.remoteAddress;
      return "";
    }

    function protocolOf(request, trust) {
      if (trust) {
        var forwarded = request.headers["x-forwarded-proto"];
        if (forwarded) return String(forwarded).split(",")[0].trim();
      }
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
        ip: ipOf(request, trustProxy),
        host: headers.host || "",
        protocol: protocolOf(request, trustProxy)
      };
    }

    // A mistake is answered.
    function fail(info, res, err) {
      if (err && err.badRequest) {
        res.status(400);
        res.send("Invalid JSON request body.");
        return;
      }
      if (err && err.tooLarge) {
        res.status(413);
        res.send("The request is too large.");
        return;
      }
      var catchers = errorHandlers.slice();
      if (!catchers.length) {
        var catcher = errorHandler || middlewareErrorHandler;
        if (catcher) catchers.push(catcher);
      }
      function fallback(error) {
        log.error(error);
        res.status(500);
        // The reason of the mistake is only told to the visitor when the
        // program says that it may, because it may tell something about the
        // machine of the server.
        if (exposeErrors) {
          res.send("Server error: " + (error && error.message ? error.message : error));
        } else {
          res.send("Server error");
        }
      }
      function dispatch(index, error) {
        if (index >= catchers.length) {
          fallback(error);
          return;
        }
        var called = false;
        function next(nextError) {
          if (called) return;
          called = true;
          dispatch(index + 1, nextError || error);
        }
        try {
          var result = catchers[index](error, info, res, next);
          if (result && typeof result.then === "function") {
            result.catch(function (nextError) { next(nextError); });
          }
        } catch (nextError) {
          dispatch(index + 1, nextError);
        }
      }
      dispatch(0, err);
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
      // The biggest frame that is read, so that a visitor cannot fill the
      // memory of the server with one message.
      var maxFrame = opts.maxFrame === undefined ? 8 * 1024 * 1024 : Number(opts.maxFrame);
      var messageHandlers = [];
      var closeHandlers = [];
      var messageOpcode = 0;
      var messageParts = [];
      var messageLength = 0;
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
        var binary = typeof Buffer !== "undefined" && Buffer.isBuffer(text);
        frame(binary ? 0x82 : 0x81, binary ? text : Buffer.from(String(text), "utf8"));
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
          var final = (first & 0x80) !== 0;
          var masked = (second & 0x80) !== 0;
          if ((first & 0x70) !== 0 || !masked) {
            close();
            return;
          }
          var length = second & 0x7f;
          var offset = 2;
          if (length === 126) {
            if (buffer.length < offset + 2) return;
            length = buffer.readUInt16BE(offset);
            offset += 2;
          } else if (length === 127) {
            if (buffer.length < offset + 8) return;
            // The length of a frame is written in eight bytes, so that a
            // frame may be bigger than four gigabytes. Both halves are
            // read, or the frame would be cut short.
            var high = buffer.readUInt32BE(offset);
            var low = buffer.readUInt32BE(offset + 4);
            length = high * 4294967296 + low;
            offset += 8;
          }
          // A frame that is too big is refused, so that a visitor cannot
          // fill the memory of the server.
          if (length > maxFrame) {
            close();
            return;
          }
          if (opcode >= 0x8 && (!final || length > 125)) {
            close();
            return;
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
          if (opcode === 0x0) {
            if (!messageOpcode) {
              close();
              return;
            }
            messageLength += payload.length;
            if (messageLength > maxFrame) {
              close();
              return;
            }
            messageParts.push(payload);
            if (!final) continue;
            payload = Buffer.concat(messageParts, messageLength);
            opcode = messageOpcode;
            messageOpcode = 0;
            messageParts = [];
            messageLength = 0;
          } else if (opcode === 0x1 || opcode === 0x2) {
            if (messageOpcode) {
              close();
              return;
            }
            if (!final) {
              messageOpcode = opcode;
              messageParts = [payload];
              messageLength = payload.length;
              continue;
            }
          } else if (opcode !== 0x8 && opcode !== 0xa) {
            close();
            return;
          }
          if (opcode === 0x1 || opcode === 0x2) {
            var message = opcode === 0x2 ? payload : payload.toString("utf8");
            for (var j = 0; j < messageHandlers.length; j++) messageHandlers[j](message);
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
      // The visitor may go away at any time. When that happens the stream
      // is closed, so that the server does not wait for it for ever.
      var ended = function () {
        if (!open) return;
        open = false;
        if (onClose) onClose();
      };
      if (response.on) {
        response.on("close", ended);
        response.on("error", ended);
      }
      if (response.socket) {
        response.socket.on("close", ended);
        response.socket.on("error", ended);
      }
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
          try { response.end(); } catch (ignored) { }
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

    // The times that the server waits for the visitor.
    if (opts.requestTimeout !== undefined) {
      serverInstance.requestTimeout = Number(opts.requestTimeout);
    }
    if (opts.headersTimeout !== undefined) {
      serverInstance.headersTimeout = Number(opts.headersTimeout);
    }
    if (opts.keepAliveTimeout !== undefined) {
      serverInstance.keepAliveTimeout = Number(opts.keepAliveTimeout);
    }

    // A mistake of the server itself must not take the whole process down
    // without a word. The port may be taken, or the socket may be broken.
    var serverError = null;
    var onServerError = null;
    serverInstance.on("error", function (err) {
      serverError = err;
      log.error("the server stopped: " + (err && err.message ? err.message : err));
      if (err && err.code === "EADDRINUSE") {
        log.error("the port " + actualPort + " is taken by another program.");
      }
      if (onServerError) {
        try { onServerError(err); } catch (ignored) { }
      }
      if (isNode) {
        try { ctx.nodeModule("node:process").exitCode = 1; } catch (ignored) { }
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
        connection.onClose(function () {
          var index = sockets.indexOf(connection);
          if (index !== -1) sockets.splice(index, 1);
        });
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

      // File helpers stream asynchronously so serving a large file does not
      // block the event loop or keep the whole body in memory.
      res.sendFile = function (file, options) {
        var settings = options || {};
        var filename = String(file);
        fs.stat(filename, function (err, info) {
          if (err) {
            if (!res.isFinished()) {
              res.status(err.code === "ENOENT" ? 404 : 500);
              res.send(err.code === "ENOENT" ? "Not found" : "Cannot read file");
            }
            return;
          }
          if (!info.isFile()) {
            res.status(404);
            res.send("Not found");
            return;
          }
          if (settings.headers) {
            for (var key in settings.headers) {
              if (Object.prototype.hasOwnProperty.call(settings.headers, key)) {
                res.set(key, settings.headers[key]);
              }
            }
          }
          if (settings.type) res.contentType(settings.type);
          res.set("Content-Length", String(info.size));
          if (method === "HEAD") {
            res.end();
            return;
          }
          var stream = fs.createReadStream(filename);
          stream.on("data", function (chunk) { res.write(chunk); });
          stream.on("end", function () { res.end(); });
          stream.on("error", function () {
            if (!res.isFinished()) {
              res.status(500);
              res.end();
            }
          });
        });
        return res;
      };
      res.download = function (file, name, options) {
        var settings = options || {};
        var filename = name === undefined ? pathModule.basename(String(file)) : String(name);
        var safeName = filename.replace(/[\r\n"\\]/g, "_");
        settings = Object.assign({}, settings, { headers: Object.assign({}, settings.headers || {}) });
        settings.headers["Content-Disposition"] = "attachment; filename=\"" + safeName + "\"";
        return res.sendFile(file, settings);
      };

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
      // A request that takes too long is refused, so that a handler that
      // waits for ever does not keep the connection open.
      if (requestTimeout > 0) {
        var timer = hostSetTimeout(function () {
          if (res.isFinished()) return;
          res.status(408);
          res.send("The request took too long.");
        }, requestTimeout);
        res.onFinish(function () { hostClearTimeout(timer); });
      }
      // The files of the folder of the server are served before the
      // routes, so that a program may put a file of its own in front of
      // them.
      if (staticDir && serveStatic(request, res, url.pathname)) return;
      // The limit of the body of this request.
      var max = limit;
      if (found && found.route.limit !== undefined) {
        max = Math.min(max, found.route.limit);
      }
      for (var pieceIndex = 0; pieceIndex < pieces.length; pieceIndex++) {
        if (pieces[pieceIndex].bodyLimit !== undefined) {
          max = Math.min(max, pieces[pieceIndex].bodyLimit);
        }
      }
      // A body that says how big it is is refused before it is read, so
      // that a large body never takes the memory of the server.
      var declared = request.headers["content-length"];
      if (declared !== undefined && Number(declared) > max) {
        var tooBig = veyError("the request is too large");
        tooBig.tooLarge = true;
        fail(makeRequest(request, url, found ? found.params : {}), res, tooBig);
        return;
      }
      readBody(request, max).then(function (body) {
        var info = makeRequest(request, url, found ? found.params : {});
        res.req = info;
        info.body = body.parsed;
        info.text = body.raw;
        info.files = body.files;
        var store = sessions.open(info.cookies, info.protocol === "https");
        // The cookie of the session is written just before the body, so
        // that it is there even when the handler ends the answer by itself.
        res.beforeFinish(function () {
          var header = sessions.header(store);
          if (header) res.set("Set-Cookie", header);
        });
        sessions.run(store, function () {
          runChain(pieces, 0, info, res, function () {
            if (res.isFinished()) return;
            // The question of the methods of an address is answered.
            if (method === "OPTIONS" && !found) {
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
              // Response helpers return `res` for chaining. Returning that
              // object from a handler must not serialize its internal state as
              // the response body, especially for asynchronous file helpers.
              if (res.isFinished() || result === res) return;
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

    var api = {
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
        var piece = typeof path === "function" ? path : fn;
        if (piece && piece.errorHandler) {
          middlewareErrorHandler = piece.errorHandler;
          errorHandlers.push(piece.errorHandler);
        }
        if (typeof path === "function") {
          middlewares.push({ path: null, fn: path });
        } else {
          middlewares.push({ path: String(path), fn: fn });
        }
        // A middle piece that asks for a smaller body makes the limit of
        // the whole server smaller, so that the body is not read at all
        // when it is too large.
        var asked = fn && fn.bodyLimit;
        if (typeof asked === "number" && isFinite(asked) && asked < limit) {
          limit = asked;
        }
        return this;
      },
      // The piece that catches the mistakes.
      error: function (handler) {
        errorHandler = handler;
        if (typeof handler === "function") errorHandlers.push(handler);
        return this;
      },
      static: function (dir, options) {
        var opts = options || {};
        staticDir = pathModule.resolve(dir);
        staticIndex = opts.index === undefined ? "index.html" : String(opts.index);
        if (opts.maxAge !== undefined) {
          staticMaxAge = Number(opts.maxAge);
        }
        return this;
      },
      notFound: function (handler) {
        notFoundHandler = handler;
        return this;
      },
      start: function (callback) {
        serverInstance.listen(actualPort, function () {
          installSignals();
          var address = serverInstance.address();
          var port = address && address.port ? address.port : actualPort;
          actualPort = port;
          // The host the server listens on, so that the line that says it
          // is listening names the right one. A server that listens on
          // every address has no name of its own.
          var host = address && address.address ? address.address : "";
          if (host === "::" || host === "0.0.0.0") host = "localhost";
          print("Vey server listening on http://" + host + ":" + port);
          if (callback) callback();
        });
        return this;
      },
      onShutdown: function (fn) {
        if (typeof fn === "function") shutdownCallbacks.push(fn);
        return this;
      },
      // The mistake of the server is given to the program, so that it may
      // answer it in its own way.
      onError: function (fn) {
        onServerError = fn;
        return this;
      },
      // The mistake of the server, when there was one.
      lastError: function () {
        return serverError;
      },
      // The port the server listens on.
      port: function () {
        var address = serverInstance.address();
        return address && address.port ? address.port : actualPort;
      },
      // The server stops, and the requests that are running are waited for.
      stop: function () {
        removeSignals();
        return new Promise(function (resolve) {
          for (var i = 0; i < sockets.length; i++) sockets[i].close();
          for (var j = 0; j < streams.length; j++) streams[j].close();
          var closed = false;
          // The time that the requests that are running may take.
          var deadline = Date.now() + (opts.shutdownTimeout === undefined
            ? 10000
            : Number(opts.shutdownTimeout));
          function finish() {
            if (closed) return;
            closed = true;
            resolve();
          }
          function wait() {
            if (running === 0) {
              serverInstance.close(finish);
              if (serverInstance.closeIdleConnections) serverInstance.closeIdleConnections();
              if (serverInstance.closeAllConnections) serverInstance.closeAllConnections();
              return;
            }
            // The requests that are running take too long, so the server
            // is closed anyway.
            if (Date.now() > deadline) {
              if (serverInstance.closeAllConnections) serverInstance.closeAllConnections();
              serverInstance.close(finish);
              return;
            }
            hostSetTimeout(wait, 20);
          }
          wait();
        });
      }
    };
    app = api;
    return api;
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
