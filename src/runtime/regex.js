// The regex, so that a text may be searched with a pattern.
function veyRegex(ctx) {
  var veyError = ctx.veyError;

  // The pattern is compiled again for every use, because a regex with the
  // "g" mark keeps the place of its last match.
  function compile(pattern, flags) {
    try {
      return new RegExp(String(pattern), flags === undefined || flags === null ? "" : String(flags));
    } catch (err) {
      throw veyError(
        "the pattern of the regex is not valid: " +
          (err && err.message ? err.message : String(err))
      );
    }
  }

  // The groups of a match, with the whole match at the front.
  function groups(found) {
    var out = [];
    for (var i = 0; i < found.length; i++) {
      out.push(found[i] === undefined ? null : found[i]);
    }
    return out;
  }

  function regex(pattern, flags) {
    var source = compile(pattern, flags);

    return {
      // The pattern and the marks of the pattern, as they were written.
      source: source.source,
      flags: source.flags,

      // Whether the text holds a match.
      test: function (text) {
        return compile(source.source, source.flags).test(String(text));
      },

      // The first match, with its groups, or nothing when there is none.
      match: function (text) {
        var found = compile(source.source, source.flags).exec(String(text));
        if (!found) return null;
        return groups(found);
      },

      // Every match of the text, each one with its groups.
      matchAll: function (text) {
        var marks = source.flags.indexOf("g") === -1 ? source.flags + "g" : source.flags;
        var re = compile(source.source, marks);
        var out = [];
        var found = re.exec(String(text));
        while (found !== null) {
          out.push(groups(found));
          // A match that is empty would keep the same place forever.
          if (found[0] === "") re.lastIndex += 1;
          found = re.exec(String(text));
        }
        return out;
      },

      // The text with the first match left out.
      replace: function (text, replacement) {
        var re = compile(source.source, source.flags.replace(/g/g, ""));
        return String(text).replace(re, replacement === undefined ? "" : replacement);
      },

      // The text with every match left out.
      replaceAll: function (text, replacement) {
        var marks = source.flags.indexOf("g") === -1 ? source.flags + "g" : source.flags;
        var re = compile(source.source, marks);
        return String(text).replace(re, replacement === undefined ? "" : replacement);
      },

      // The parts of the text that the pattern cuts apart.
      split: function (text) {
        return String(text).split(compile(source.source, source.flags));
      }
    };
  }

  ctx.regex = regex;

  return {
    regex: regex,
  };
}
