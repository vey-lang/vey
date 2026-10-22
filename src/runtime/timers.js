// The timers of the browser: a page that waits, polls and draws.
function veyTimers(ctx) {
  var scope = ctx.scope;
  var doc = ctx.doc;
  var veyError = ctx.veyError;
  var hostSetTimeout = ctx.hostSetTimeout;
  var hostClearTimeout = ctx.hostClearTimeout;
  var hostSetInterval = ctx.hostSetInterval;
  var hostClearInterval = ctx.hostClearInterval;

  // The window of the page, which is where the frames are drawn.
  function window() {
    if (doc && doc.defaultView) return doc.defaultView;
    return scope;
  }

  // The call of a timer, once the time is up.
  function setTimeout(fn, ms) {
    return hostSetTimeout(fn, ms === undefined ? 0 : ms);
  }

  // The call of a timer is taken back.
  function clearTimeout(id) {
    hostClearTimeout(id);
  }

  // The call of a timer, again and again.
  function setInterval(fn, ms) {
    return hostSetInterval(fn, ms === undefined ? 0 : ms);
  }

  // The call of a timer is taken back.
  function clearInterval(id) {
    hostClearInterval(id);
  }

  // The call of a frame, when the page is drawn again.
  function requestAnimationFrame(fn) {
    var view = window();
    if (!view || !view.requestAnimationFrame) {
      throw veyError("requestAnimationFrame() is only available in the browser.");
    }
    return view.requestAnimationFrame(function (time) { fn(time); });
  }

  // The call of a frame is taken back.
  function cancelAnimationFrame(id) {
    var view = window();
    if (!view || !view.cancelAnimationFrame) {
      throw veyError("cancelAnimationFrame() is only available in the browser.");
    }
    view.cancelAnimationFrame(id);
  }

  ctx.setTimeout = setTimeout;
  ctx.clearTimeout = clearTimeout;
  ctx.setInterval = setInterval;
  ctx.clearInterval = clearInterval;
  ctx.requestAnimationFrame = requestAnimationFrame;
  ctx.cancelAnimationFrame = cancelAnimationFrame;

  return {
    setTimeout: setTimeout,
    clearTimeout: clearTimeout,
    setInterval: setInterval,
    clearInterval: clearInterval,
    requestAnimationFrame: requestAnimationFrame,
    cancelAnimationFrame: cancelAnimationFrame,
  };
}
