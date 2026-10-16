// Adds the jest-dom matchers (toBeInTheDocument, toHaveTextContent, ...) to
// vitest's expect.
import "@testing-library/jest-dom/vitest";
import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";

// Vitest globals are off, so React Testing Library cannot register its
// automatic cleanup; do it explicitly so each test starts with an empty DOM.
afterEach(() => {
  cleanup();
});
