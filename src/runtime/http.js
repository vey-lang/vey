// The HTTP client.
function veyHttp(ctx) {
  var scope = ctx.scope;
  var veyError = ctx.veyError;
  var hostSetTimeout = ctx.hostSetTimeout;
  var hostClearTimeout = ctx.hostClearTimeout;

  // The headers of a request, as the host wants them.
  function plainHeaders(headers) {
    if (!headers) return headers;
    if (typeof headers.get === "function" && typeof headers.entries === "function") {
      var out = {};
      var pairs = headers.entries();
      for (var i = 0; i < pairs.length; i++) out[pairs[i][0]] = pairs[i][1];
      return out;
    }
    return headers;
  }

  // A request whose answer is the same `Response` that `fetch` gives, with
  // the body already read: a JSON answer is a value, anything else is text.
  function request(url, options) {
    var fetchFn = scope.fetch || (typeof fetch !== "undefined" ? fetch : null);
    if (!fetchFn) return Promise.reject(veyError("fetch is not available"));
    var opts = Object.assign({}, options || {});
    if (opts.headers) opts.headers = plainHeaders(opts.headers);
    return fetchFn(url, opts).then(function (response) {
      return response.text().then(function (text) {
        var contentType = response.headers.get("content-type") || "";
        var body = text;
        if (contentType.indexOf("json") !== -1) {
          try { body = JSON.parse(text); } catch (err) { body = text; }
        }
        return Response(body, {
          status: response.status,
          statusText: response.statusText,
          headers: response.headers,
          url: response.url || url,
          text: text
        });
      });
    });
  }

  // The headers of a request or of an answer.
  function Headers(init) {
    var store = {};
    var order = [];

    function lower(name) { return String(name).toLowerCase(); }

    function put(name, value) {
      var key = lower(name);
      if (!(key in store)) order.push(key);
      store[key] = [String(value)];
    }

    function add(name, value) {
      var key = lower(name);
      if (!(key in store)) {
        order.push(key);
        store[key] = [];
      }
      store[key].push(String(value));
    }

    function read(name) {
      var key = lower(name);
      if (!(key in store)) return null;
      return store[key].join(", ");
    }

    function remove(name) {
      var key = lower(name);
      delete store[key];
      var index = order.indexOf(key);
      if (index !== -1) order.splice(index, 1);
    }

    function pairs() {
      var out = [];
      for (var i = 0; i < order.length; i++) {
        out.push([order[i], store[order[i]].join(", ")]);
      }
      return out;
    }

    if (init) {
      if (Array.isArray(init)) {
        for (var i = 0; i < init.length; i++) put(init[i][0], init[i][1]);
      } else if (typeof init.forEach === "function") {
        // The headers of the host, or any other map.
        init.forEach(function (value, key) { put(key, value); });
      } else {
        for (var key in init) {
          if (Object.prototype.hasOwnProperty.call(init, key)) put(key, init[key]);
        }
      }
    }

    return {
      get: read,
      getAll: function (name) {
        var key = lower(name);
        return key in store ? store[key].slice() : [];
      },
      has: function (name) { return lower(name) in store; },
      set: put,
      append: add,
      delete: remove,
      entries: pairs,
      keys: function () {
        return pairs().map(function (pair) { return pair[0]; });
      },
      values: function () {
        return pairs().map(function (pair) { return pair[1]; });
      },
      forEach: function (fn) {
        var list = pairs();
        for (var i = 0; i < list.length; i++) fn(list[i][1], list[i][0]);
      }
    };
  }

  // A request that is written before it is sent.
  function Request(url, options) {
    var opts = options || {};
    var headers = opts.headers;
    if (!headers || typeof headers.get !== "function") headers = Headers(headers);
    return {
      url: String(url),
      method: String(opts.method === undefined ? "GET" : opts.method).toUpperCase(),
      headers: headers,
      body: opts.body === undefined ? null : opts.body,
      signal: opts.signal || null
    };
  }

  // An answer of a server.
  function Response(body, options) {
    var opts = options || {};
    var status = opts.status === undefined ? 200 : Number(opts.status);
    var headers = opts.headers;
    if (!headers || typeof headers.get !== "function") headers = Headers(headers);
    // A response may be given its body as a value and the text it was read
    // from, so that `body` is a value and `text()` is the text itself.
    var text;
    if (opts.text !== undefined) {
      text = String(opts.text);
    } else if (body === undefined || body === null) {
      text = "";
    } else {
      text = typeof body === "string" ? body : String(body);
    }
    var answer = {
      status: status,
      statusText: opts.statusText === undefined ? "" : String(opts.statusText),
      ok: status >= 200 && status < 300,
      headers: headers,
      url: opts.url === undefined ? "" : String(opts.url),
      redirected: false,
      body: body === undefined ? null : body,
      text: function () {
        return Promise.resolve(text);
      },
      json: function () {
        try {
          return Promise.resolve(JSON.parse(text));
        } catch (err) {
          return Promise.reject(err);
        }
      },
      clone: function () {
        return Response(body, opts);
      }
    };
    return answer;
  }

  // The end of a request, from the hand of the program.
  function AbortController() {
    var ctor = scope.AbortController;
    if (ctor) {
      // The host has one of its own, which the requests of the host know.
      var real = new ctor();
      return {
        signal: real.signal,
        abort: function (reason) {
          real.abort(reason);
        }
      };
    }
    var listeners = [];
    var signal = {
      aborted: false,
      reason: null,
      addEventListener: function (event, handler) {
        if (String(event) !== "abort") return;
        listeners.push(handler);
      },
      removeEventListener: function (event, handler) {
        if (String(event) !== "abort") return;
        var index = listeners.indexOf(handler);
        if (index !== -1) listeners.splice(index, 1);
      }
    };
    return {
      signal: signal,
      abort: function (reason) {
        if (signal.aborted) return;
        signal.aborted = true;
        signal.reason = reason === undefined ? null : reason;
        var list = listeners.slice();
        for (var i = 0; i < list.length; i++) {
          list[i]({ type: "abort", target: signal });
        }
      }
    };
  }

  function wait(ms) {
    return new Promise(function (resolve) {
      hostSetTimeout(resolve, ms);
    });
  }

  // Whether the body of a request is written by the browser itself.
  function isFormBody(value) {
    if (!value || typeof value !== "object") return false;
    var names = ["FormData", "Blob", "URLSearchParams", "ArrayBuffer"];
    for (var i = 0; i < names.length; i++) {
      var ctor = scope[names[i]];
      if (ctor && value instanceof ctor) return true;
    }
    return false;
  }

  // A request to a server, with the time it may take and the times it may
  // be tried again.
  function fetch(url, options) {
    var opts = Object.assign({}, options || {});
    if (opts.headers) opts.headers = plainHeaders(opts.headers);
    var fetchFn = scope.fetch || (typeof fetch !== "undefined" ? fetch : null);
    if (!fetchFn) return Promise.reject(veyError("fetch is not available"));

    var timeout = opts.timeout === undefined ? 0 : Number(opts.timeout);
    var retries = opts.retries === undefined ? 0 : Number(opts.retries);
    var retryDelay = opts.retryDelay === undefined ? 0 : Number(opts.retryDelay);
    var controller = null;
    var timer = null;

    // A request that may be ended needs a signal of its own.
    if (!opts.signal && (timeout > 0 || retries > 0)) {
      controller = AbortController();
      opts.signal = controller.signal;
    }
    if (timeout > 0 && controller) {
      timer = hostSetTimeout(function () { controller.abort(); }, timeout);
    }

    function attempt(remaining) {
      return fetchFn(url, opts).then(function (answer) {
        // A server that is busy may be asked again.
        if (answer.status >= 500 && remaining > 0) {
          return wait(retryDelay).then(function () { return attempt(remaining - 1); });
        }
        return answer.text().then(function (text) {
          // A JSON answer is a value, anything else is a text. The answer
          // of `http.get` is the same.
          var contentType = answer.headers.get("content-type") || "";
          var body = text;
          if (contentType.indexOf("json") !== -1) {
            try { body = JSON.parse(text); } catch (err) { body = text; }
          }
          return Response(body, {
            status: answer.status,
            statusText: answer.statusText,
            headers: answer.headers,
            url: answer.url,
            text: text
          });
        });
      }, function (err) {
        if (remaining > 0) {
          return wait(retryDelay).then(function () { return attempt(remaining - 1); });
        }
        throw err;
      });
    }

    var waiting = attempt(retries);
    if (!timer) return waiting;
    return waiting.then(function (answer) {
      hostClearTimeout(timer);
      return answer;
    }, function (err) {
      hostClearTimeout(timer);
      throw err;
    });
  }

  var http = {
    get: function (url, options) {
      return request(url, options);
    },
    post: function (url, body, options) {
      // Copy the caller's options so that the request never mutates them.
      var opts = Object.assign({}, options || {});
      var headers = Object.assign({}, plainHeaders(opts.headers) || {});
      if (typeof body === "string") {
        headers["content-type"] = headers["content-type"] || "application/json";
        opts.body = body;
      } else if (isFormBody(body)) {
        // A form or a file is written by the browser, which knows the marks
        // of the body itself.
        opts.body = body;
      } else {
        headers["content-type"] = headers["content-type"] || "application/json";
        opts.body = JSON.stringify(body);
      }
      opts.method = "POST";
      opts.headers = headers;
      return request(url, opts);
    },
    // A request with the time it may take and the times it may be tried
    // again.
    fetch: fetch,
    // The pieces of a request.
    Request: Request,
    Response: Response,
    Headers: Headers,
    // The end of a request.
    AbortController: AbortController
  };

  ctx.http = http;
  ctx.fetch = fetch;
  ctx.Request = Request;
  ctx.Response = Response;
  ctx.Headers = Headers;
  ctx.AbortController = AbortController;

  return {
    http: http,
    fetch: fetch,
    Request: Request,
    Response: Response,
    Headers: Headers,
    AbortController: AbortController,
  };
}
