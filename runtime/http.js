// The HTTP client.
function veyHttp(ctx) {
  var scope = ctx.scope;

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
        ctx.http = http;

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

  return {
    http: http,
  };
}
