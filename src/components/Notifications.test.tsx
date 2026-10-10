import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
} from "@testing-library/react";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../i18n";
import {
    NOTIFICATION_DURATION_MS,
    NotificationProvider,
    useNotify,
} from "./Notifications";

beforeEach(() => {
    vi.useFakeTimers();
});
afterEach(() => {
    cleanup();
    vi.useRealTimers();
});

/** A caller that notifies from a button, like a feature after a save. */
function Trigger({ message }: { message: string }) {
    const notify = useNotify();
    return (
        <button
            type="button"
            onClick={() => {
                notify(message);
            }}
        >
            Trigger
        </button>
    );
}

function notificationCard(): HTMLElement {
    const status = screen.getByRole("status");
    const card = status.parentElement?.parentElement;
    if (!(card instanceof HTMLElement)) {
        throw new Error("notification card missing");
    }
    return card;
}

async function mount(locale: "en" | "zh-CN" = "en", message = "Saved Work.") {
    const instance = await createAppI18n(locale);
    render(
        <StrictMode>
            <I18nextProvider i18n={instance}>
                <NotificationProvider>
                    <Trigger message={message} />
                </NotificationProvider>
            </I18nextProvider>
        </StrictMode>,
    );
    const trigger = screen.getByRole("button", { name: "Trigger" });
    trigger.focus();
    fireEvent.click(trigger);
    return trigger;
}

function advance(ms: number): void {
    act(() => {
        vi.advanceTimersByTime(ms);
    });
}

test("shows the message in a status region without taking focus, then dismisses itself", async () => {
    const trigger = await mount();
    expect(screen.getByRole("status").textContent).toBe("Saved Work.");
    expect(document.activeElement).toBe(trigger);
    advance(NOTIFICATION_DURATION_MS - 1);
    expect(screen.getByText("Saved Work.")).toBeDefined();
    advance(1);
    expect(screen.queryByText("Saved Work.")).toBeNull();
    // The live region stays mounted so the next message is announced too.
    expect(screen.getByRole("status").textContent).toBe("");
});

test("hovering pauses the timer and leaving resumes the time left", async () => {
    await mount();
    advance(1000);
    const card = notificationCard();
    fireEvent.mouseEnter(card);
    advance(NOTIFICATION_DURATION_MS * 3);
    expect(screen.getByText("Saved Work.")).toBeDefined();
    fireEvent.mouseLeave(card);
    advance(NOTIFICATION_DURATION_MS - 1000 - 1);
    expect(screen.getByText("Saved Work.")).toBeDefined();
    advance(1);
    expect(screen.queryByText("Saved Work.")).toBeNull();
});

test("focus inside pauses the timer; closing restores focus to where it came from", async () => {
    const trigger = await mount();
    const close = screen.getByRole("button", { name: "Dismiss notification" });
    fireEvent.focus(close, { relatedTarget: trigger });
    close.focus();
    advance(NOTIFICATION_DURATION_MS * 3);
    expect(screen.getByText("Saved Work.")).toBeDefined();
    fireEvent.click(close);
    expect(screen.queryByText("Saved Work.")).toBeNull();
    expect(document.activeElement).toBe(trigger);
});

test("Escape closes it", async () => {
    await mount();
    fireEvent.keyDown(
        screen.getByRole("button", { name: "Dismiss notification" }),
        {
            key: "Escape",
        },
    );
    expect(screen.queryByText("Saved Work.")).toBeNull();
});

test("the close button is named in Chinese and a new message replaces the old one", async () => {
    const trigger = await mount("zh-CN", "已保存 工作。");
    expect(screen.getByRole("button", { name: "关闭通知" })).toBeDefined();
    advance(NOTIFICATION_DURATION_MS - 500);
    // Notifying again restarts the full duration for the new message.
    fireEvent.click(trigger);
    advance(NOTIFICATION_DURATION_MS - 1);
    expect(screen.getAllByText("已保存 工作。")).toHaveLength(1);
    advance(1);
    expect(screen.queryByText("已保存 工作。")).toBeNull();
});

test("the live region holds only the message text; the close button is outside it", async () => {
    await mount();
    const status = screen.getByRole("status");
    const close = screen.getByRole("button", { name: "Dismiss notification" });
    expect(status.textContent).toBe("Saved Work.");
    expect(status.children).toHaveLength(0);
    expect(status.contains(close)).toBe(false);
    // Both still sit in the same card, side by side.
    expect(close.parentElement).toBe(notificationCard());
    fireEvent.click(close);
    // The region stays mounted and empty, without the button.
    expect(screen.getByRole("status")).toBe(status);
    expect(status.textContent).toBe("");
    expect(
        screen.queryByRole("button", { name: "Dismiss notification" }),
    ).toBeNull();
});

test("closing while hovered does not leave the next notification paused", async () => {
    const trigger = await mount();
    const card = notificationCard();
    fireEvent.mouseEnter(card);
    fireEvent.click(
        screen.getByRole("button", { name: "Dismiss notification" }),
    );
    fireEvent.click(trigger);
    advance(NOTIFICATION_DURATION_MS);
    expect(screen.queryByText("Saved Work.")).toBeNull();
});
