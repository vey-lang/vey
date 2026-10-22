// The runtime of Vey, shared by Node and the browser.
//
// The parts of the runtime live in the files next to this one. They are
// joined together by the compiler, in the order of `src/runtime.rs`, and
// every part hands the helpers it knows to the ones that come after it.
//
// Nothing here is in the scope of the page: the parts live inside the
// function that is created below.

function createVeyRuntime(scope, req) {
  scope = scope || globalThis;
  req = req || null;

  // Node is detected from the `require` hook that the CLI passes in, not from
  // the ambient `process` global: bundlers and test runners often define a
  // `process` shim in the browser, which would wrongly unlock Node-only
  // helpers such as `fs` and `path`.
  var ctx = {
    scope: scope,
    req: req,
    isNode: typeof req === "function",
    doc: scope.document || null,
    log: scope.console || console
  };

  // The helpers that every part may use.
  veyShared(ctx);

  var V = {};
  V = Object.assign(V, veyNode(ctx));
  V = Object.assign(V, veyCore(ctx));
  V = Object.assign(V, veyText(ctx));
  V = Object.assign(V, veyCollections(ctx));
  V = Object.assign(V, veyMap(ctx));
  V = Object.assign(V, veyEnum(ctx));
  V = Object.assign(V, veyData(ctx));
  V = Object.assign(V, veyDate(ctx));
  V = Object.assign(V, veyWeb(ctx));
  V = Object.assign(V, veyRegex(ctx));
  V = Object.assign(V, veyIntl(ctx));
  V = Object.assign(V, veyBytes(ctx));
  V = Object.assign(V, veyState(ctx));
  V = Object.assign(V, veyDom(ctx));
  V = Object.assign(V, veyMount(ctx));
  V = Object.assign(V, veyEvents(ctx));
  V = Object.assign(V, veyHistory(ctx));
  V = Object.assign(V, veyRouter(ctx));
  V = Object.assign(V, veyTimers(ctx));
  V = Object.assign(V, veyStorage(ctx));
  V = Object.assign(V, veyFiles(ctx));
  V = Object.assign(V, veySocket(ctx));
  V = Object.assign(V, veyObserve(ctx));
  V = Object.assign(V, veyHttp(ctx));
  V = Object.assign(V, veyHighlight(ctx));
  V = Object.assign(V, veySession(ctx));
  V = Object.assign(V, veyMiddleware(ctx));
  V = Object.assign(V, veyServer(ctx));

  return V;
}
