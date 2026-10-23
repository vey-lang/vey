// The middle pieces of the server: the log of the requests, the CORS, the
// compression, the limit of a request and the catch of the mistakes.
function veyMiddleware(ctx) {
  var log = ctx.log;
  var isNode = ctx.isNode;
  var nodeModule = ctx.nodeModule;
  var veyError = ctx.veyError;

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
      res.append("Vary", "Origin");
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
  //
  // The kinds of body that are already compressed are left alone, and the
  // answer says that it depends on what the visitor can read, so that a
  // cache in between does not give the compressed form to a visitor that
  // cannot read it.
  var COMPRESSED = [
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/avif",
    "video/",
    "audio/",
    "application/zip",
    "application/gzip",
    "application/x-gzip",
    "application/x-compressed",
    "application/pdf"
  ];

  function compress(options) {
    var opts = options || {};
    var threshold = opts.threshold === undefined ? 1024 : Number(opts.threshold);
    return function (req, res, next) {
      if (!isNode) return next();
      var accept = req.headers["accept-encoding"] || "";
      if (accept.indexOf("gzip") === -1) return next();
      var zlib = nodeModule("node:zlib");
      // The answer depends on what the visitor can read.
      res.append("Vary", "Accept-Encoding");
      res.transform(function (body) {
        // A body that is already compressed does not become smaller.
        var kind = res.get("Content-Type") || "";
        for (var i = 0; i < COMPRESSED.length; i++) {
          if (kind.indexOf(COMPRESSED[i]) === 0) return body;
        }
        var text = typeof body === "string" ? body : null;
        // The size is counted in bytes, not in letters, so that a text of
        // many letters is compressed even when it has few characters.
        var size = text === null ? body.length : Buffer.byteLength(text, "utf8");
        if (size < threshold) return body;
        var out = zlib.gzipSync(text === null ? body : text);
        res.set("Content-Encoding", "gzip");
        res.remove("Content-Length");
        return out;
      });
      next();
    };
  }

  // The marks that keep a browser from doing things it should not.
  //
  // The marks are the ones that a server of a page needs, so that the
  // page of the program is not used to do something else.
  function security(options) {
    var opts = options || {};
    return function (req, res, next) {
      res.set("X-Content-Type-Options", "nosniff");
      res.set("X-Frame-Options", opts.frame === false ? "ALLOWALL" : "SAMEORIGIN");
      res.set("Referrer-Policy", opts.referrer || "strict-origin-when-cross-origin");
      res.set("X-DNS-Prefetch-Control", "off");
      if (opts.hsts !== false) {
        res.set("Strict-Transport-Security", "max-age=31536000; includeSubDomains");
      }
      if (opts.csp !== false) {
        res.set(
          "Content-Security-Policy",
          opts.csp || "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'"
        );
      }
      if (opts.permissions !== false) {
        res.set(
          "Permissions-Policy",
          opts.permissions || "geolocation=(), microphone=(), camera=()"
        );
      }
      next();
    };
  }

  // The limit of the requests of one visitor.
  //
  // The requests of a visitor are counted, and the ones that are too
  // many are refused with the code 429.
  function rateLimit(options) {
    var opts = options || {};
    var limit = opts.limit === undefined ? (opts.perMinute || 60) : Number(opts.limit);
    var window = opts.window === undefined ? 60000 : Number(opts.window);
    var counts = {};
    return function (req, res, next) {
      var key = req.ip || req.headers["x-forwarded-for"] || "unknown";
      var now = Date.now();
      var entry = counts[key];
      if (!entry || now - entry.started > window) {
        entry = { started: now, count: 0 };
        counts[key] = entry;
      }
      entry.count += 1;
      var left = Math.max(0, limit - entry.count);
      res.set("X-RateLimit-Limit", String(limit));
      res.set("X-RateLimit-Remaining", String(left));
      res.set("X-RateLimit-Reset", String(Math.ceil((entry.started + window) / 1000)));
      if (entry.count > limit) {
        res.set("Retry-After", String(Math.ceil((entry.started + window - now) / 1000)));
        res.status(429);
        res.send("Too many requests.");
        return;
      }
      next();
    };
  }

  // The CSRF: a request that changes something must come from the page
  // of the program.
  //
  // The token is given to the page, and it must come back in the head
  // `X-CSRF-Token`.
  function csrf(options) {
    var opts = options || {};
    var name = opts.header || "X-CSRF-Token";
    var cookieName = opts.cookie || "vey.csrf";
    var safe = { GET: 1, HEAD: 1, OPTIONS: 1, TRACE: 1 };
    return function (req, res, next) {
      if (safe[req.method]) {
        next();
        return;
      }
      var cookies = req.cookies || {};
      var token = cookies[cookieName];
      var given = req.headers[name.toLowerCase()];
      if (!token || !given || token !== given) {
        res.status(403);
        res.send("The request is not from this page.");
        return;
      }
      next();
    };
  }

  // The catch of the mistakes.
  //
  // It is the first middle piece of the server, so that it may catch the
  // mistakes of the ones that come after it, and of the route itself.
  function errorHandler(handler) {
    var piece = function (req, res, next) {
      next();
    };
    piece.errorHandler = handler;
    return piece;
  }

  // The limit of the size of a request.
  //
  // The limit is known by the server, so that a body that is too large is
  // refused before it is read, and not after.
  function bodyLimit(limit) {
    var max = Number(limit);
    function piece(req, res, next) {
      var length = req.headers["content-length"];
      if (length !== undefined && Number(length) > max) {
        res.status(413);
        res.send("The request is too large.");
        return;
      }
      next();
    }
    piece.bodyLimit = max;
    return piece;
  }

  function validationError(path, message) {
    var error = veyError("Invalid " + path + ": " + message);
    error.badRequest = true;
    error.validation = true;
    return error;
  }

  function matchesRule(value, rule, path) {
    if (typeof rule === "string") rule = { type: rule };
    if (!rule || typeof rule !== "object") return null;
    if ((value === undefined || value === null) && rule.required !== false) {
      return validationError(path, "is required");
    }
    if (value === undefined || value === null) return null;
    var type = rule.type;
    if (type === "array" && !Array.isArray(value)) return validationError(path, "must be an array");
    if (type === "object" && (typeof value !== "object" || Array.isArray(value))) return validationError(path, "must be an object");
    if (type === "string" && typeof value !== "string") return validationError(path, "must be a string");
    if (type === "number" && (typeof value !== "number" || !isFinite(value))) return validationError(path, "must be a number");
    if (type === "boolean" && typeof value !== "boolean") return validationError(path, "must be a boolean");
    if (rule.enum && rule.enum.indexOf(value) === -1) return validationError(path, "has an invalid value");
    if (rule.min !== undefined && value.length !== undefined && value.length < rule.min) return validationError(path, "is too short");
    if (rule.max !== undefined && value.length !== undefined && value.length > rule.max) return validationError(path, "is too long");
    if (rule.pattern && typeof value === "string" && !(new RegExp(rule.pattern).test(value))) return validationError(path, "has an invalid format");
    if (rule.fields && typeof value === "object" && !Array.isArray(value)) {
      for (var key in rule.fields) {
        var nested = matchesRule(value[key], rule.fields[key], path + "." + key);
        if (nested) return nested;
      }
    }
    return null;
  }

  function validate(schema) {
    var definition = schema || {};
    return function (req, res, next) {
      var sources = ["body", "query", "params"];
      for (var sourceIndex = 0; sourceIndex < sources.length; sourceIndex++) {
        var source = sources[sourceIndex];
        if (!definition[source]) continue;
        var error = matchesRule(req[source], definition[source], source);
        if (error) return next(error);
      }
      next();
    };
  }

  ctx.logger = logger;
  ctx.cors = cors;
  ctx.compress = compress;
  ctx.errorHandler = errorHandler;
  ctx.bodyLimit = bodyLimit;
  ctx.validate = validate;
  ctx.security = security;
  ctx.rateLimit = rateLimit;
  ctx.csrf = csrf;

  return {
    logger: logger,
    cors: cors,
    compress: compress,
    errorHandler: errorHandler,
    bodyLimit: bodyLimit,
    validate: validate,
    security: security,
    rateLimit: rateLimit,
    csrf: csrf,
  };
}
