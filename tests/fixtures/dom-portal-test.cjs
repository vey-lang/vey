// A page that writes a part of itself in another place.
//
// The part that is written elsewhere follows the state of the page, and it
// is taken away when the page does not hold it any more.

const bundle = process.argv[2];
const { side, waitFor } = require("./dom-fake.cjs");

require(bundle);

waitFor(
  () => side.textContent.includes("first 2"),
  () => "the portal is not in the other place: " + side.textContent,
  () =>
    waitFor(
      () => side.textContent.includes("second 3"),
      () => "the portal did not follow the state: " + side.textContent,
      () =>
        waitFor(
          () => side.textContent === "",
          () => "the portal stayed behind: " + side.textContent,
          () => console.log("DOM_PORTAL_TEST_OK")
        )
    )
);
