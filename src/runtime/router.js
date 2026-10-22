// The routes of the browser: the address of the page and the page it shows.
function veyRouter(ctx) {
  var doc = ctx.doc;
  var scope = ctx.scope;
  var veyError = ctx.veyError;
  var safeDecode = ctx.safeDecode;
  var matchRoute = ctx.matchRoute;
  var history = ctx.history;
  var renderMounted = ctx.renderMounted;

  var routes = [];
  var listening = false;
  var current = { path: "/", params: {}, query: {}, view: null };

  // The address of the page, as it is written.
  function base() {
    var view = (doc && doc.defaultView) || scope;
    if (view && view.location && view.location.href) return view.location.href;
    return "http://localhost/";
  }

  function address(text) {
    try {
      return new URL(String(text), base());
    } catch (err) {
      return new URL(String(text), "http://localhost/");
    }
  }

  // The parameters of a path, when the pattern of the route fits it.
  function match(pattern, pathname) {
    return matchRoute(pattern, pathname);
  }

  // The route of a path, when there is one.
  function routeOf(pathname) {
    for (var i = 0; i < routes.length; i++) {
      var params = match(routes[i].path, pathname);
      if (params) return { route: routes[i], params: params };
    }
    return null;
  }

  // The page of a route is written in the page.
  function show(pathname, search) {
    var parsed = address(pathname + (search || ""));
    var query = {};
    parsed.searchParams.forEach(function (value, key) { query[key] = value; });
    var found = routeOf(parsed.pathname);
    current = {
      path: parsed.pathname,
      params: found ? found.params : {},
      query: query,
      view: found ? found.route.view : null
    };
    if (!found) return null;
    var view = found.route.view;
    var tree = typeof view === "function" ? view(found.params, query) : view;
    renderMounted(tree);
    return tree;
  }

  // The address of the browser is watched, so that the way back shows the
  // page of that address.
  function listen() {
    if (listening) return;
    listening = true;
    var view = (doc && doc.defaultView) || scope;
    if (!view || !view.addEventListener) return;
    view.addEventListener("popstate", function () {
      var location = view.location;
      if (!location) return;
      show(location.pathname, location.search);
    });
  }

  // A route of the program.
  //
  // The pattern of the path may hold a parameter, such as "/users/{id}", and
  // the view is called with the parameters of the path.
  function route(path, view) {
    if (!doc) {
      throw veyError("route() is only available in the browser.");
    }
    var entry = { path: String(path), view: view };
    entry.remove = function () {
      var index = routes.indexOf(entry);
      if (index !== -1) routes.splice(index, 1);
    };
    routes.push(entry);
    listen();
    // The route that fits the address of the page is shown at once.
    var location = (doc && doc.defaultView && doc.defaultView.location) || scope.location;
    if (location) show(location.pathname, location.search);
    return entry;
  }

  // Another address is shown.
  function navigate(url) {
    if (!doc) {
      throw veyError("navigate() is only available in the browser.");
    }
    var parsed = address(url);
    var query = {};
    parsed.searchParams.forEach(function (value, key) { query[key] = value; });
    var found = routeOf(parsed.pathname);
    current = {
      path: parsed.pathname,
      params: found ? found.params : {},
      query: query,
      view: found ? found.route.view : null
    };
    history.push({ vey: true }, "", parsed.pathname + parsed.search + parsed.hash);
    if (!found) return null;
    var view = found.route.view;
    var tree = typeof view === "function" ? view(found.params, query) : view;
    renderMounted(tree);
    return tree;
  }

  // The route that is shown now, with its parameters.
  function currentRoute() {
    return {
      path: current.path,
      params: current.params,
      query: current.query
    };
  }

  // The parameters of the route that is shown now.
  function routeParams() {
    return current.params;
  }

  ctx.route = route;
  ctx.navigate = navigate;
  ctx.currentRoute = currentRoute;
  ctx.routeParams = routeParams;

  return {
    route: route,
    navigate: navigate,
    currentRoute: currentRoute,
    routeParams: routeParams,
  };
}
