// The helpers that the other parts share.
function veyShared(ctx) {
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

  ctx.flatten = flatten;
  ctx.ownProps = ownProps;
  ctx.toList = toList;
  ctx.safeDecode = safeDecode;
  ctx.toText = toText;
  ctx.cssPropertyName = cssPropertyName;
  ctx.domStyleName = domStyleName;
  ctx.veyError = veyError;
  ctx.escapeHtml = escapeHtml;

  return {};
}
