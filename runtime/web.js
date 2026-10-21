// The helpers a web page needs: the query of an address and the cookies of
// a request.
function veyWeb(ctx) {
  var safeDecode = ctx.safeDecode;
  var ownProps = ctx.ownProps;

  // The query of an address, such as "a=1&b=2".
  var query = {
    parse: function (text) {
      var out = {};
      var source = String(text === undefined || text === null ? "" : text);
      if (source.charAt(0) === "?") source = source.slice(1);
      if (!source) return out;
      source.split("&").forEach(function (part) {
        if (!part) return;
        var index = part.indexOf("=");
        var key = index === -1 ? part : part.slice(0, index);
        var value = index === -1 ? "" : part.slice(index + 1);
        out[safeDecode(key.replace(/\+/g, " "))] = safeDecode(value.replace(/\+/g, " "));
      });
      return out;
    },
    stringify: function (object) {
      var parts = [];
      var source = object || {};
      for (var key in ownProps(source)) {
        var value = source[key];
        if (value === null || value === undefined) continue;
        if (Array.isArray(value)) {
          value.forEach(function (item) {
            parts.push(encodeURIComponent(key) + "=" + encodeURIComponent(item));
          });
        } else {
          parts.push(encodeURIComponent(key) + "=" + encodeURIComponent(value));
        }
      }
      return parts.join("&");
    }
  };

  // The cookies of a request, and the cookies of a response.
  var cookie = {
    parse: function (header) {
      var out = {};
      var source = String(header === undefined || header === null ? "" : header);
      if (!source) return out;
      source.split(";").forEach(function (part) {
        var text = part.trim();
        if (!text) return;
        var index = text.indexOf("=");
        if (index === -1) {
          out[safeDecode(text)] = "";
          return;
        }
        out[safeDecode(text.slice(0, index).trim())] = safeDecode(text.slice(index + 1).trim());
      });
      return out;
    },
    stringify: function (object) {
      var parts = [];
      var source = object || {};
      for (var key in ownProps(source)) {
        var value = source[key];
        if (value === null || value === undefined) continue;
        parts.push(encodeURIComponent(key) + "=" + encodeURIComponent(value));
      }
      return parts.join("; ");
    }
  };

  ctx.query = query;
  ctx.cookie = cookie;

  return {
    query: query,
    cookie: cookie,
  };
}
