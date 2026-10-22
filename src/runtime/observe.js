// The screen of the browser: the size of the window, the place of an element
// on the page and the size of an element.
function veyObserve(ctx) {
  var scope = ctx.scope;
  var doc = ctx.doc;
  var veyError = ctx.veyError;

  // Whether the screen of the visitor is of the given size.
  //
  // The answer is an object with `matches`, which says whether the query is
  // true at this moment, and `addEventListener`, which is called when the
  // answer changes.
  function matchMedia(query) {
    var view = (doc && doc.defaultView) || scope;
    if (!view.matchMedia) {
      throw veyError("matchMedia() is only available in the browser.");
    }
    return view.matchMedia(String(query));
  }

  // The watcher of the place of the elements on the page.
  function IntersectionObserver(callback, options) {
    var view = (doc && doc.defaultView) || scope;
    if (!view.IntersectionObserver) {
      throw veyError("IntersectionObserver is only available in the browser.");
    }
    if (options === undefined) return new view.IntersectionObserver(callback);
    return new view.IntersectionObserver(callback, options);
  }

  // The watcher of the size of the elements.
  function ResizeObserver(callback, options) {
    var view = (doc && doc.defaultView) || scope;
    if (!view.ResizeObserver) {
      throw veyError("ResizeObserver is only available in the browser.");
    }
    if (options === undefined) return new view.ResizeObserver(callback);
    return new view.ResizeObserver(callback, options);
  }

  ctx.matchMedia = matchMedia;
  ctx.IntersectionObserver = IntersectionObserver;
  ctx.ResizeObserver = ResizeObserver;

  return {
    matchMedia: matchMedia,
    IntersectionObserver: IntersectionObserver,
    ResizeObserver: ResizeObserver,
  };
}
