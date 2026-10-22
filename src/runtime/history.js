// The history of the browser: the address of the page and the way back.
function veyHistory(ctx) {
  var scope = ctx.scope;
  var doc = ctx.doc;
  var veyError = ctx.veyError;

  // The history of the browser, when the page has one.
  function historyOf() {
    if (!doc) {
      throw veyError("history is only available in the browser.");
    }
    var view = doc.defaultView || scope;
    if (!view.history) {
      throw veyError("history is not available in this browser.");
    }
    return view.history;
  }

  var history = {
    // A new address is written, and the way back to the old one is kept.
    push: function (state, title, url) {
      var book = historyOf();
      book.pushState(
        state === undefined ? null : state,
        title === undefined ? "" : String(title),
        url === undefined ? null : String(url)
      );
    },

    // A new address is written, and the way back to the old one is
    // forgotten.
    replace: function (state, title, url) {
      var book = historyOf();
      book.replaceState(
        state === undefined ? null : state,
        title === undefined ? "" : String(title),
        url === undefined ? null : String(url)
      );
    },

    // The page before this one is shown again.
    back: function () {
      historyOf().back();
    },

    // The page after this one is shown again.
    forward: function () {
      historyOf().forward();
    },

    // The page that is a number of steps away is shown again.
    go: function (n) {
      historyOf().go(n === undefined ? 0 : Number(n));
    },

    // The state that belongs to the address of the page.
    state: function () {
      return historyOf().state;
    }
  };

  ctx.history = history;

  return {
    history: history,
  };
}
