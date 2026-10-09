import { StrictMode } from "react";
import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import App from "./App";
import { type AppInfo, getAppInfo } from "./lib/desktop";

// Replace only the desktop boundary; the component and React lifecycle remain real.
vi.mock(import("./lib/desktop"), () => ({
  getAppInfo: vi.fn<typeof getAppInfo>(),
}));

beforeEach(() => {
  vi.mocked(getAppInfo).mockReset();
});

afterEach(() => {
  // Vitest globals are disabled, so cleanup is registered explicitly.
  cleanup();
});

/** Hold a desktop response until the test chooses to complete it. */
function pendingAppInfo(): {
  promise: Promise<AppInfo | null>;
  resolve: (value: AppInfo | null) => void;
  reject: (error: Error) => void;
} {
  let resolveResponse: (value: AppInfo | null) => void = () => {
    throw new Error("The pending response has not been initialized.");
  };
  let rejectResponse: (error: Error) => void = () => {
    throw new Error("The pending response has not been initialized.");
  };
  const promise = new Promise<AppInfo | null>((resolve, reject) => {
    // The executor runs synchronously, before these callbacks are returned.
    resolveResponse = resolve;
    rejectResponse = reject;
  });
  return { promise, resolve: resolveResponse, reject: rejectResponse };
}

test("replaces the loading status with desktop metadata when the request completes", async () => {
  const response = pendingAppInfo();
  vi.mocked(getAppInfo).mockReturnValue(response.promise);
  render(<App />);

  expect(screen.getByRole("status").textContent).toBe(
    "Checking desktop runtime…",
  );
  await act(async () => {
    response.resolve({ name: "vibemate", version: "0.1.0" });
    await response.promise;
  });
  expect(screen.getByRole("status").textContent).toBe(
    "vibemate 0.1.0 · Desktop runtime ready",
  );
});

test("reports a failed desktop request without displaying the underlying error", async () => {
  vi.mocked(getAppInfo).mockRejectedValue(
    new Error("Synthetic private diagnostic"),
  );
  render(<App />);

  await screen.findByText(
    "Desktop metadata unavailable. Restart the app to retry.",
  );
  expect(screen.getByRole("status").textContent).toBe(
    "Desktop metadata unavailable. Restart the app to retry.",
  );
  expect(screen.queryByText("Synthetic private diagnostic")).toBeNull();
});

test("reports browser preview when no desktop runtime is available", async () => {
  vi.mocked(getAppInfo).mockResolvedValue(null);
  render(<App />);

  await screen.findByText("Browser preview · Desktop runtime unavailable");
  expect(screen.getByRole("status").textContent).toBe(
    "Browser preview · Desktop runtime unavailable",
  );
});

test.each(["success", "failure"] as const)(
  "keeps the current result when a cleaned-up request later settles with %s",
  async (outcome) => {
    const obsolete = pendingAppInfo();
    const current = pendingAppInfo();
    vi.mocked(getAppInfo)
      .mockReturnValueOnce(obsolete.promise)
      .mockReturnValueOnce(current.promise);

    // StrictMode cleans up the first effect before starting it again. Both
    // responses address the same visible component, so a missing guard is observable.
    render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    await act(async () => {
      current.resolve({ name: "vibemate", version: "0.1.0" });
      await current.promise;
    });
    expect(screen.getByRole("status").textContent).toBe(
      "vibemate 0.1.0 · Desktop runtime ready",
    );

    await act(async () => {
      if (outcome === "success") {
        obsolete.resolve({ name: "Obsolete", version: "0.0.9" });
        await obsolete.promise;
      } else {
        obsolete.reject(new Error("Obsolete request failed"));
        await expect(obsolete.promise).rejects.toThrow(
          "Obsolete request failed",
        );
      }
    });
    expect(screen.getByRole("status").textContent).toBe(
      "vibemate 0.1.0 · Desktop runtime ready",
    );
  },
);
