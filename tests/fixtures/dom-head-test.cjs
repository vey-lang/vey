// The head of the page, the values the components share and the component
// that catches the mistakes of the ones under it.

const bundle = process.argv[2];
const { root, document, waitFor } = require("./dom-fake.cjs");

require(bundle);

const headText = () => document.head.textContent;

waitFor(
  () => headText().includes("A page of Vey"),
  () => "the title is not in the head: " + headText(),
  () =>
    waitFor(
      () =>
        root.textContent.includes("theme dark") &&
        root.textContent.includes("the fallback") &&
        // The mark of the head is not in the body of the page.
        !root.textContent.includes("A page of Vey"),
      () => "the page is not right: " + root.textContent,
      () => {
        const meta = document.head.children.find(
          (child) => child.tagName === "META"
        );
        if (!meta || meta.attributes.content !== "a description") {
          throw new Error("the meta mark is not in the head: " + headText());
        }
        console.log("DOM_HEAD_TEST_OK");
      }
    )
);
