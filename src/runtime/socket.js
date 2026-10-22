// The sockets of the browser: a socket that stays open and the events of
// the server.
function veySocket(ctx) {
  var scope = ctx.scope;
  var doc = ctx.doc;
  var veyError = ctx.veyError;

  // A socket that stays open, so that the page and the server may talk.
  function WebSocket(url, protocols) {
    var ctor = scope.WebSocket;
    if (!ctor) {
      throw veyError("WebSocket is only available in the browser.");
    }
    if (protocols === undefined) return new ctor(String(url));
    return new ctor(String(url), protocols);
  }

  // The events of the server, as they come.
  function EventSource(url, options) {
    var ctor = scope.EventSource;
    if (!ctor) {
      throw veyError("EventSource is only available in the browser.");
    }
    if (options === undefined) return new ctor(String(url));
    return new ctor(String(url), options);
  }

  ctx.WebSocket = WebSocket;
  ctx.EventSource = EventSource;

  return {
    WebSocket: WebSocket,
    EventSource: EventSource,
  };
}
