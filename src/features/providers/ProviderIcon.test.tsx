import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { ProviderIcon } from "./ProviderIcon";

afterEach(cleanup);

// Which brand file a known kind renders is appearance, checked by eye in the
// app and the browser preview. What is worth a test is that a kind this build
// does not know — including one inherited from Object.prototype — renders
// nothing and does not throw.
test.each(["future-provider", "toString"])(
    "%s renders no icon and does not throw",
    (kind) => {
        const { container } = render(<ProviderIcon kind={kind} />);
        expect(container.innerHTML).toBe("");
    },
);
