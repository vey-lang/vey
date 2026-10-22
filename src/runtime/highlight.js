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

  // The names of the types of the language.
  var types = ["number", "string", "bool", "any", "null", "void", "never"];

  // The tags of HTML, which are the names that the markup of a program may
  // use.
  //
  // The list is the one of the parser, so that a name that is not a tag of
  // HTML is never painted as one.
  var htmlTags = (
    "a abbr address article aside audio b bdi bdo blockquote br button canvas " +
    "caption cite code col colgroup data datalist dd del details dfn dialog " +
    "div dl dt em fieldset figcaption figure footer form h1 h2 h3 h4 h5 h6 " +
    "header hgroup hr html head i iframe img input ins kbd label legend li " +
    "link main mark menu meta meter nav noscript ol optgroup option output p " +
    "picture pre progress q s samp script section select small span strong " +
    "style sub summary sup table tbody td textarea tfoot th thead time tr u " +
    "ul var video wbr"
  ).split(" ");

  // The marks of the language that are not names.
  //
  // The longer marks come first, so that the shorter ones do not hide them.
  var operators = [
    "...", "?.", "??", "=>", "|>", "->", "**", "==", "!=", "<=", ">=", "&&",
    "||", "+=", "-=", "*=", "/=", "%=", "=", "+", "-", "*", "/", "%", "<", ">",
    "!", "?"
  ];

  function escape(text) {
    return String(text)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  // A piece of the code, in the colour of its kind.
  function paint(kind, text) {
    return '<span class="' + kind + '">' + escape(text) + "</span>";
  }

  function isKeyword(word) {
    return keywords.indexOf(word) !== -1;
  }

  function isType(word) {
    return types.indexOf(word) !== -1;
  }

  function isTag(word) {
    return htmlTags.indexOf(word) !== -1;
  }

  // A name that begins with a big letter is a component or a type.
  function isBig(word) {
    var first = word.charAt(0);
    return first === first.toUpperCase() && first !== first.toLowerCase();
  }

  // The mark that follows a name, with the space in between.
  function markAfter(text, index) {
    while (index < text.length && /[ \t]/.test(text.charAt(index))) index += 1;
    return text.charAt(index);
  }

  // The code of a line as HTML, with the parts of the language coloured.
  function highlight(source) {
    var text = String(source === undefined || source === null ? "" : source);
    var out = "";
    var index = 0;
    // Whether the line begins with a name, which may be a tag of the markup.
    var atLineStart = true;
    // Whether a brace was the last thing that was read, so that the name
    // that follows it may be a tag as well.
    var afterBrace = false;
    // Whether the name of a tag was read, so that the names that follow it
    // are its classes, its id and its attributes.
    var inTag = false;
    // The last thing that was read, a name or a mark.
    var before = "";
    // The last name that was read.
    var lastWord = "";

    while (index < text.length) {
      var c = text.charAt(index);

      // The rest of the line is a comment.
      if (c === "/" && text.charAt(index + 1) === "/") {
        var end = text.indexOf("\n", index);
        if (end === -1) end = text.length;
        out += paint("cm", text.slice(index, end));
        index = end;
        continue;
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
        out += paint("str", part);
        atLineStart = false;
        afterBrace = false;
        before = "";
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
        out += paint("num", number);
        atLineStart = false;
        afterBrace = false;
        before = "";
        continue;
      }

      // A name.
      if (/[A-Za-z_$]/.test(c)) {
        var word = "";
        while (index < text.length && /[A-Za-z0-9_$]/.test(text.charAt(index))) {
          word += text.charAt(index);
          index += 1;
        }
        // The mark that follows the name.
        var after = markAfter(text, index);
        // Whether the name stands where a tag of the markup may stand.
        var markup = atLineStart || afterBrace;

        if (isKeyword(word)) {
          out += paint("kw", word);
        } else if (isType(word)) {
          out += paint("ty", word);
        } else if (isBig(word)) {
          // A name that begins with a big letter is a component, or a type
          // when it follows the name of a field.
          if (before === ":" || lastWord === "enum" || lastWord === "type") {
            out += paint("ty", word);
          } else {
            out += paint("cp", word);
          }
          if (markup) inTag = true;
        } else if (markup && isTag(word)) {
          // A name where a tag of the markup may stand is a tag.
          out += paint("tag", word);
          inTag = true;
        } else if (inTag && before === ".") {
          // The class of a tag.
          out += paint("cls", word);
        } else if (inTag && before === "#") {
          // The id of a tag.
          out += paint("at", word);
        } else if (inTag && after === "=") {
          // The name of an attribute of a tag.
          out += paint("at", word);
        } else if (after === "(") {
          out += paint("fn", word);
        } else {
          out += escape(word);
        }

        atLineStart = false;
        afterBrace = false;
        before = word;
        lastWord = word;
        continue;
      }

      // A mark of the language.
      var mark = "";
      for (var i = 0; i < operators.length; i++) {
        if (text.substr(index, operators[i].length) === operators[i]) {
          mark = operators[i];
          break;
        }
      }
      if (mark) {
        out += paint("op", mark);
        index += mark.length;
        atLineStart = false;
        afterBrace = false;
        before = mark.charAt(mark.length - 1);
        continue;
      }

      // Anything else.
      if (c === "\n") {
        atLineStart = true;
        afterBrace = false;
        inTag = false;
      } else if (c === "{") {
        afterBrace = true;
        inTag = false;
      } else if (c !== " " && c !== "\t") {
        afterBrace = false;
        before = c;
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
