// The page of a program that reads a signal.
//
// The page is written again when the signal changes, without the program
// writing it again by hand.

const bundle = process.argv[2];
const { root, waitFor } = require("./dom-fake.cjs");

require(bundle);

waitFor(
  () => root.textContent.includes("Clicked 1 times"),
  () => "the page did not follow the signal: " + root.textContent,
  () => console.log("DOM_SIGNAL_TEST_OK")
);
