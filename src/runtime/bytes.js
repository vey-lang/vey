// The binary: the bytes of a text, and the ways to write them down.
function veyBytes(ctx) {
  var utf8Encode = ctx.utf8Encode;
  var utf8Decode = ctx.utf8Decode;
  var toBytes = ctx.toBytes;

  var ALPHABET =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

  // The bytes of a text in base64.
  function toBase64(list) {
    var bytes = toBytes(list);
    var out = "";
    for (var i = 0; i < bytes.length; i += 3) {
      var first = bytes[i] & 0xff;
      var second = i + 1 < bytes.length ? bytes[i + 1] & 0xff : 0;
      var third = i + 2 < bytes.length ? bytes[i + 2] & 0xff : 0;
      out += ALPHABET.charAt(first >> 2);
      out += ALPHABET.charAt(((first & 3) << 4) | (second >> 4));
      out += i + 1 < bytes.length ? ALPHABET.charAt(((second & 15) << 2) | (third >> 6)) : "=";
      out += i + 2 < bytes.length ? ALPHABET.charAt(third & 63) : "=";
    }
    return out;
  }

  // The bytes of a text in base64.
  function fromBase64(text) {
    var source = String(text === undefined || text === null ? "" : text)
      .replace(/[^A-Za-z0-9+/=]/g, "");
    // The padding at the end is not a part of the bytes.
    while (source.length && source.charAt(source.length - 1) === "=") {
      source = source.slice(0, source.length - 1);
    }
    var out = [];
    for (var i = 0; i < source.length; i += 4) {
      var first = ALPHABET.indexOf(source.charAt(i));
      var second = ALPHABET.indexOf(source.charAt(i + 1));
      var third = source.charAt(i + 2) === "" ? 0 : ALPHABET.indexOf(source.charAt(i + 2));
      var fourth = source.charAt(i + 3) === "" ? 0 : ALPHABET.indexOf(source.charAt(i + 3));
      out.push((((first << 2) | (second >> 4)) & 0xff));
      if (source.length > i + 2) {
        out.push(((((second & 15) << 4) | (third >> 2)) & 0xff));
      }
      if (source.length > i + 3) {
        out.push(((((third & 3) << 6) | fourth) & 0xff));
      }
    }
    return out;
  }

  // The bytes of a text in hexadecimal numbers.
  function toHex(list) {
    var bytes = toBytes(list);
    var out = "";
    for (var i = 0; i < bytes.length; i++) {
      var value = bytes[i] & 0xff;
      out += (value < 16 ? "0" : "") + value.toString(16);
    }
    return out;
  }

  // The bytes of a text in hexadecimal numbers.
  function fromHex(text) {
    var source = String(text === undefined || text === null ? "" : text);
    var out = [];
    for (var i = 0; i + 1 < source.length; i += 2) {
      var value = parseInt(source.substr(i, 2), 16);
      if (isNaN(value)) continue;
      out.push(value & 0xff);
    }
    return out;
  }

  var bytes = {
    // The bytes of a text.
    //
    // The encoding may be "utf8", "latin1", "hex" or "base64".
    from: function (text, encoding) {
      var kind = String(encoding === undefined || encoding === null ? "utf8" : encoding).toLowerCase();
      if (kind === "base64") return fromBase64(text);
      if (kind === "hex") return fromHex(text);
      if (kind === "latin1" || kind === "binary" || kind === "ascii") {
        var source = String(text === undefined || text === null ? "" : text);
        var out = [];
        for (var i = 0; i < source.length; i++) out.push(source.charCodeAt(i) & 0xff);
        return out;
      }
      return utf8Encode(text);
    },

    // The bytes of a text in base64.
    toBase64: function (value) {
      return toBase64(value);
    },

    // The bytes of a text in base64.
    fromBase64: function (text) {
      return fromBase64(text);
    },

    // One list of bytes out of many.
    concat: function (list) {
      var out = [];
      var items = list || [];
      for (var i = 0; i < items.length; i++) {
        var part = toBytes(items[i]);
        for (var j = 0; j < part.length; j++) out.push(part[j]);
      }
      return out;
    },

    // The number of the bytes.
    length: function (value) {
      return toBytes(value).length;
    },

    // The bytes of a text in hexadecimal numbers.
    hex: function (value) {
      return toHex(value);
    },

    // The text of the bytes.
    text: function (value) {
      return utf8Decode(value);
    }
  };

  ctx.bytes = bytes;

  return {
    bytes: bytes,
  };
}
