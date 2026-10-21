// The colouring of the code, so that a sample of the language may be shown
// with the same colours as the page of an error.

function veyHighlight(ctx) {
  // The words of the language.
  var keywords = [
    "let", "const", "fn", "return", "if", "else", "while", "for", "in", "repeat",
    "break", "continue", "try", "catch", "finally", "use", "as", "export",
    "component", "await", "async", "true", "false", "null", "and", "or", "not",
    "match", "enum", "type"
  ];

  function escape(text) {
    return String(text)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  function isKeyword(word) {
    return keywords.indexOf(word) !== -1;
  }

  // The code of a line as HTML, with the parts of the language coloured.
  function highlight(source) {
    var text = String(source === undefined || source === null ? "" : source);
    var out = "";
    var index = 0;

    while (index < text.length) {
      var c = text.charAt(index);

      // The rest of the line is a comment.
      if (c === "/" && text.charAt(index + 1) === "/") {
        out += '<span class="cm">' + escape(text.slice(index)) + "</span>";
        break;
      }

      // A text, with the interpolations inside it.
      if (c === '"' || c === "'") {
        var quote = c;
        var part = c;
        index += 1;
        while (index < text.length) {
          var next = text.charAt(index);
          part += next;
          index += 1;
          if (next === "\\") {
            if (index < text.length) {
              part += text.charAt(index);
              index += 1;
            }
            continue;
          }
          if (next === quote) break;
        }
        out += '<span class="str">' + escape(part) + "</span>";
        continue;
      }

      // A number.
      if (c >= "0" && c <= "9") {
        var number = "";
        while (index < text.length) {
          var digit = text.charAt(index);
          if ((digit >= "0" && digit <= "9") || digit === ".") {
            number += digit;
            index += 1;
          } else {
            break;
          }
        }
        out += '<span class="num">' + escape(number) + "</span>";
        continue;
      }

      // A word.
      if (/[A-Za-z_$]/.test(c)) {
        var word = "";
        while (index < text.length && /[A-Za-z0-9_$]/.test(text.charAt(index))) {
          word += text.charAt(index);
          index += 1;
        }
        if (isKeyword(word)) {
          out += '<span class="kw">' + escape(word) + "</span>";
        } else if (text.charAt(index) === "(") {
          out += '<span class="fn">' + escape(word) + "</span>";
        } else {
          out += escape(word);
        }
        continue;
      }

      out += escape(c);
      index += 1;
    }

    return out;
  }

  ctx.highlight = highlight;

  return {
    highlight: highlight,
  };
}
