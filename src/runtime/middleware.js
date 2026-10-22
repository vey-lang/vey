// The middle pieces of the server: the log of the requests, the CORS, the
// compression, the limit of a request and the catch of the mistakes.
function veyMiddleware(ctx) {
  var log = ctx.log;
  var isNode = ctx.isNode;
  var nodeModule = ctx.nodeModule;

  // The log of the requests.
  //
  // The line is written when the answer is sent, so that the time of the
  // request is in it.
  function logger(options) {
    var opts = options || {};
    return function (req, res, next) {
      var started = Date.now();
      res.onFinish(function () {
        if (opts.silent) return;
        var line = req.method + " " + req.path + " " + res.getStatus() +
          " " + (Date.now() - started) + "ms";
        log.log(line);
      });
      next();
    };
  }

  // The CORS: the page of another address may ask this server.
  function cors(options) {
    var opts = options || {};
    return function (req, res, next) {
      var origin = opts.origin === undefined ? "*" : opts.origin;
      res.set("Access-Control-Allow-Origin", origin);
      res.set("Vary", "Origin");
      if (opts.credentials) res.set("Access-Control-Allow-Credentials", "true");
      res.set(
        "Access-Control-Allow-Methods",
        opts.methods || "GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS"
      );
      var asked = req.headers["access-control-request-headers"];
      if (asked) res.set("Access-Control-Allow-Headers", asked);
      res.set("Access-Control-Max-Age", String(opts.maxAge === undefined ? 600 : opts.maxAge));
      next();
    };
  }

  // The compression of the answer.
  //
  // The body is compressed only when the visitor says that it may be, and
  // only when it is worth it.
  function compress(options) {
    var opts = options || {};
    var threshold = opts.threshold === undefined ? 1024 : Number(opts.threshold);
    return function (req, res, next) {
      if (!isNode) return next();
      var accept = req.headers["accept-encoding"] || "";
      if (accept.indexOf("gzip") === -1) return next();
      var zlib = nodeModule("node:zlib");
      res.transform(function (body) {
        var text = typeof body === "string" ? body : null;
        if (text !== null && text.length < threshold) return body;
        var out = zlib.gzipSync(text === null ? body : text);
        res.set("Content-Encoding", "gzip");
        res.remove("Content-Length");
        return out;
      });
      next();
    };
  }

  // The catch of the mistakes.
  //
  // It is the first middle piece of the server, so that it may catch the
  // mistakes of the ones that come after it.
  function errorHandler(handler) {
    return function (req, res, next) {
      try {
        next();
      } catch (err) {
        if (handler) {
          handler(err, req, res);
          return;
        }
        log.error(err);
        res.status(500);
        res.send("Server error: " + (err && err.message ? err.message : err));
      }
    };
  }

  // The limit of the size of a request.
  function bodyLimit(limit) {
    var max = Number(limit);
    return function (req, res, next) {
      var length = req.headers["content-length"];
      if (length !== undefined && Number(length) > max) {
        res.status(413);
        res.send("The request is too large.");
        return;
      }
      next();
    };
  }

  ctx.logger = logger;
  ctx.cors = cors;
  ctx.compress = compress;
  ctx.errorHandler = errorHandler;
  ctx.bodyLimit = bodyLimit;

  ctx.middlewares = {
    logger: logger,
    cors: cors,
    compress: compress,
    errorHandler: errorHandler,
    bodyLimit: bodyLimit,
  };

  return {
    logger: logger,
    cors: cors,
    compress: compress,
    errorHandler: errorHandler,
    bodyLimit: bodyLimit,
  };
}
