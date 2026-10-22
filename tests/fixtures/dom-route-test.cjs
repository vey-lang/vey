// The route of the page.
//
// The route is written before the page is put out, and the page of the
// route is shown anyway.

const bundle = process.argv[2];
const { root, waitFor } = require("./dom-fake.cjs");

global.location.pathname = "/users/7";
global.location.href = "http://localhost/users/7";

require(bundle);

waitFor(
  () => root.textContent.includes("User 7"),
  () => "the route was not shown: " + root.textContent,
  () => console.log("DOM_ROUTE_TEST_OK")
);
