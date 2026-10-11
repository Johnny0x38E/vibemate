import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/react";
import { StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import { setFieldSelectValue } from "../../test/fieldSelect";
import {
    McpRequestError,
    saveMcpDefinition,
    type McpRecord,
} from "../../lib/desktop/mcp";
import { McpForm } from "./McpForm";
vi.mock(import("../../lib/desktop/mcp"), async (importOriginal) => ({
    ...(await importOriginal()),
    saveMcpDefinition: vi.fn(),
}));
const save = vi.mocked(saveMcpDefinition);
const row: McpRecord = {
    id: "0123456789abcdef0123456789abcdef",
    displayName: "Tools",
    serverName: "tools",
    enabled: true,
    revision: 1,
    connection: {
        type: "stdio",
        command: "not-a-real-server",
        args: ["--stdio"],
        cwd: null,
        env: ["TOKEN"],
    },
    createdAtMs: 1,
    updatedAtMs: 1,
    cleanupPending: false,
};
async function setup(
    record: McpRecord | null = null,
    locale: "en" | "zh-CN" = "en",
) {
    const i18n = await createAppI18n(locale);
    const onSaved = vi.fn();
    const onCancel = vi.fn();
    const onReload = vi.fn();
    const tree = (hidden: boolean) => (
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <McpForm
                    record={record}
                    hidden={hidden}
                    onSaved={onSaved}
                    onCancel={onCancel}
                    onReload={onReload}
                />
            </I18nextProvider>
        </StrictMode>
    );
    const view = render(tree(false));
    return {
        ...view,
        i18n,
        onSaved,
        onCancel,
        onReload,
        hide: () => {
            view.rerender(tree(true));
        },
    };
}
function enterBase(): void {
    fireEvent.change(screen.getByLabelText(/^Name$/), {
        target: { value: "Tools" },
    });
    fireEvent.change(screen.getByLabelText(/^Executable/), {
        target: { value: "not-a-real-server" },
    });
}
function clickAdd(index: 0 | 1): void {
    const buttons = screen.getAllByRole("button", {
        name: /^Add (argument|environment variable)$/,
    });
    const button = buttons[index];
    if (button === undefined) {
        throw new Error(`Expected Add button at index ${String(index)}.`);
    }
    fireEvent.click(button);
}
function addArg(value: string, index = 1): void {
    clickAdd(0);
    fireEvent.change(screen.getByLabelText(`Argument ${String(index)}`), {
        target: { value },
    });
}
function addField(): void {
    clickAdd(1);
    fireEvent.change(screen.getByLabelText("Field name 1"), {
        target: { value: "TOKEN" },
    });
    fireEvent.change(screen.getByLabelText(/^Value 1/), {
        target: { value: "synthetic-token" },
    });
}
function submit(): void {
    fireEvent.click(
        screen.getByRole("button", { name: /^(Save|Saving definition…)$/ }),
    );
}
beforeEach(() => {
    save.mockReset().mockResolvedValue(row);
});
afterEach(() => {
    cleanup();
});

test("creates a stdio definition with ordered literal args and clears credential input", async () => {
    const { onSaved } = await setup();
    enterBase();
    addField();
    addArg("--flag", 1);
    addArg("path with spaces", 2);
    submit();
    expect(screen.getByLabelText<HTMLInputElement>(/^Value 1/).value).toBe("");
    await waitFor(() => {
        expect(onSaved).toHaveBeenCalledWith(row);
    });
    expect(save).toHaveBeenCalledWith({
        id: null,
        expectedRevision: null,
        displayName: "Tools",
        serverName: "tools",
        enabled: true,
        connection: {
            type: "stdio",
            command: "not-a-real-server",
            args: ["--flag", "path with spaces"],
            cwd: null,
            env: [{ name: "TOKEN", value: "synthetic-token" }],
        },
    });
});

test("edits retain configured values and untouched empty arguments without exposing secrets", async () => {
    const { onSaved } = await setup({
        ...row,
        connection: {
            type: "stdio",
            command: "not-a-real-server",
            args: [""],
            cwd: null,
            env: ["TOKEN"],
        },
    });
    expect(screen.getByLabelText<HTMLInputElement>(/^Value 1/).value).toBe("");
    expect(screen.getByText(/Configured\. Leave blank/)).toBeDefined();
    submit();
    await waitFor(() => {
        expect(onSaved).toHaveBeenCalled();
    });
    expect(save.mock.calls[0]?.[0].connection).toMatchObject({
        args: [""],
        env: [{ name: "TOKEN", value: null }],
    });
});

test.each([
    {
        locale: "en" as const,
        argument: "Argument 1",
        removeArgument: "Remove argument 1",
        field: "Field name 1",
        removeField: "Remove field 1",
    },
    {
        locale: "zh-CN" as const,
        argument: "参数 1",
        removeArgument: "删除参数 1",
        field: "字段名 1",
        removeField: "删除字段 1",
    },
])(
    "icon removal actions keep localized names and remove only their row ($locale)",
    async (labels) => {
        await setup(row, labels.locale);
        fireEvent.click(
            screen.getByRole("button", { name: labels.removeArgument }),
        );
        expect(screen.queryByLabelText(labels.argument)).toBeNull();
        expect(screen.getByLabelText(labels.field)).toBeDefined();
        fireEvent.click(
            screen.getByRole("button", { name: labels.removeField }),
        );
        expect(screen.queryByLabelText(labels.field)).toBeNull();
        expect(save).not.toHaveBeenCalled();
    },
);

test.each([
    {
        locale: "en" as const,
        addArgument: "Add argument",
        argument: "Argument 2",
        addEnv: "Add environment variable",
        field: "Field name 2",
        addHeader: "Add header",
        firstField: "Field name 1",
    },
    {
        locale: "zh-CN" as const,
        addArgument: "添加参数",
        argument: "参数 2",
        addEnv: "添加环境变量",
        field: "字段名 2",
        addHeader: "添加请求头",
        firstField: "字段名 1",
    },
])(
    "localized plus buttons add argument, env and header rows without saving ($locale)",
    async (labels) => {
        const view = await setup(row, labels.locale);
        fireEvent.click(
            screen.getByRole("button", { name: labels.addArgument }),
        );
        expect(screen.getByLabelText(labels.argument)).toBeDefined();
        fireEvent.click(screen.getByRole("button", { name: labels.addEnv }));
        expect(screen.getByLabelText(labels.field)).toBeDefined();
        expect(save).not.toHaveBeenCalled();
        view.unmount();
        await setup(
            {
                ...row,
                connection: {
                    type: "http",
                    url: "https://example.com/mcp",
                    headers: [],
                },
            },
            labels.locale,
        );
        fireEvent.click(screen.getByRole("button", { name: labels.addHeader }));
        expect(screen.getByLabelText(labels.firstField)).toBeDefined();
        expect(save).not.toHaveBeenCalled();
    },
);

test("removed env fields are omitted from the save", async () => {
    await setup(row);
    fireEvent.click(screen.getByRole("button", { name: "Remove field 1" }));
    submit();
    await waitFor(() => {
        expect(save).toHaveBeenCalled();
    });
    expect(save.mock.calls[0]?.[0].connection).toMatchObject({ env: [] });
});

test("HTTP edits preserve headers and send no mixed stdio fields", async () => {
    const http: McpRecord = {
        ...row,
        connection: {
            type: "http",
            url: "https://example.com/mcp",
            headers: ["authorization"],
        },
    };
    await setup(http);
    fireEvent.change(screen.getByLabelText(/^Value 1/), {
        target: { value: "Bearer synthetic" },
    });
    submit();
    await waitFor(() => {
        expect(save).toHaveBeenCalled();
    });
    expect(save.mock.calls[0]?.[0].connection).toEqual({
        type: "http",
        url: "https://example.com/mcp",
        headers: [{ name: "authorization", value: "Bearer synthetic" }],
    });
});

test("transport switches clear typed values instead of retaining hidden credentials", async () => {
    await setup();
    addField();
    await setFieldSelectValue(
        screen.getByRole("combobox", { name: "Transport" }),
        "http",
    );
    await setFieldSelectValue(
        screen.getByRole("combobox", { name: "Transport" }),
        "stdio",
    );
    expect(screen.getByLabelText<HTMLInputElement>(/^Value 1/).value).toBe("");
});

test("hiding clears secrets but keeps non-secret draft fields", async () => {
    const view = await setup();
    enterBase();
    addField();
    view.hide();
    expect(screen.getByLabelText<HTMLInputElement>(/^Value 1/).value).toBe("");
    await waitFor(() => {
        expect(screen.getByLabelText<HTMLInputElement>(/^Value 1/).value).toBe(
            "",
        );
    });
    expect(screen.getByLabelText<HTMLInputElement>(/^Name$/).value).toBe(
        "Tools",
    );
});

test("pending requests cannot be submitted or cancelled twice", async () => {
    let resolve: (value: McpRecord) => void = () => {};
    save.mockImplementation(
        () =>
            new Promise((done) => {
                resolve = done;
            }),
    );
    const { onCancel, onSaved } = await setup();
    enterBase();
    const saveButton = screen.getByRole("button", { name: "Save" });
    submit();
    expect(screen.getByRole("button", { name: "Saving definition…" })).toBe(
        saveButton,
    );
    expect(saveButton.getAttribute("aria-disabled")).toBe("true");
    submit();
    fireEvent.click(
        screen.getByRole("button", { name: "Back to MCP servers" }),
    );
    expect(save).toHaveBeenCalledOnce();
    expect(onCancel).not.toHaveBeenCalled();
    await act(async () => {
        resolve(row);
        await Promise.resolve();
    });
    expect(onSaved).toHaveBeenCalledOnce();
});

test("unknown acknowledgments block retry and Back also requires reload", async () => {
    save.mockRejectedValue(new McpRequestError("outcome_unknown"));
    const { onReload, onCancel } = await setup();
    enterBase();
    addField();
    submit();
    await screen.findByRole("alert");
    submit();
    expect(save).toHaveBeenCalledOnce();
    expect(screen.getByLabelText<HTMLInputElement>(/^Value 1/).value).toBe("");
    fireEvent.click(
        screen.getByRole("button", { name: "Reload before retrying" }),
    );
    expect(onReload).toHaveBeenCalledOnce();
    fireEvent.click(
        screen.getByRole("button", { name: "Back to MCP servers" }),
    );
    expect(onReload).toHaveBeenCalledTimes(2);
    expect(onCancel).not.toHaveBeenCalled();
});

test("field errors are associated with their input and permit corrected retry", async () => {
    save.mockRejectedValueOnce(new McpRequestError("command_invalid"));
    await setup();
    enterBase();
    submit();
    await screen.findByRole("alert");
    const command = screen.getByLabelText<HTMLInputElement>(/^Executable/);
    expect(command.getAttribute("aria-invalid")).toBe("true");
    expect(command.getAttribute("aria-describedby")).toBe(
        screen.getByRole("alert").id,
    );
    fireEvent.change(command, { target: { value: "new-server" } });
    submit();
    await waitFor(() => {
        expect(save).toHaveBeenCalledTimes(2);
    });
});

test("unmount discards late replies without saving a frontend fixture", async () => {
    let resolve: (value: McpRecord) => void = () => {};
    save.mockImplementation(
        () =>
            new Promise((done) => {
                resolve = done;
            }),
    );
    const { unmount, onSaved } = await setup();
    enterBase();
    submit();
    unmount();
    await act(async () => {
        resolve(row);
        await Promise.resolve();
    });
    expect(onSaved).not.toHaveBeenCalled();
});

test.each(["en", "zh-CN"] as const)(
    "footer save keeps corrected input and exposes failures in %s",
    async (locale) => {
        save.mockRejectedValueOnce(new McpRequestError("command_invalid"));
        const { onSaved } = await setup(row, locale);
        const saveButton = screen.getByRole("button", {
            name: locale === "en" ? "Save" : "保存",
        });
        const footer = saveButton.closest("footer");
        expect(footer).not.toBeNull();
        fireEvent.click(saveButton);
        const alert = await screen.findByRole("alert");
        expect(footer?.contains(alert)).toBe(true);
        const command = screen.getByLabelText(
            locale === "en" ? "Executable" : "可执行程序",
        );
        fireEvent.change(command, { target: { value: "corrected-command" } });
        fireEvent.click(saveButton);
        await waitFor(() => {
            expect(onSaved).toHaveBeenCalledOnce();
        });
        expect(save).toHaveBeenCalledTimes(2);
        expect(save.mock.calls[1]?.[0].connection).toMatchObject({
            command: "corrected-command",
        });
    },
);

test("language switching preserves draft input and translates feedback", async () => {
    save.mockRejectedValue(new McpRequestError("command_invalid"));
    const { i18n } = await setup();
    enterBase();
    await act(async () => {
        await i18n.changeLanguage("zh-CN");
    });
    expect(screen.getByLabelText<HTMLInputElement>("名称").value).toBe("Tools");
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("程序名");
});

test("create derives the server identifier from the display name", async () => {
    await setup();
    enterBase();
    submit();
    await waitFor(() => {
        expect(save).toHaveBeenCalled();
    });
    expect(save.mock.calls[0]?.[0].serverName).toBe("tools");
});

test("create retries with a suffix when the derived identifier is taken", async () => {
    save.mockRejectedValueOnce(new McpRequestError("server_name_taken"));
    save.mockResolvedValueOnce(row);
    await setup();
    enterBase();
    submit();
    await waitFor(() => {
        expect(save).toHaveBeenCalledTimes(2);
    });
    expect(save.mock.calls[0]?.[0].serverName).toBe("tools");
    expect(save.mock.calls[1]?.[0].serverName).toBe("tools-2");
});

test.each(["en", "zh-CN"] as const)(
    "create and edit hide the internal identifier while preserving save behavior in %s",
    async (locale) => {
        await setup(null, locale);
        expect(screen.queryByText(/Server identifier|服务器标识/)).toBeNull();
        cleanup();
        await setup(row, locale);
        expect(screen.queryByText(/Server identifier|服务器标识/)).toBeNull();
        fireEvent.change(
            screen.getByLabelText(locale === "en" ? "Name" : "名称"),
            {
                target: { value: "Renamed tools" },
            },
        );
        fireEvent.click(
            screen.getByRole("button", {
                name: locale === "en" ? "Save" : "保存",
            }),
        );
        await waitFor(() => {
            expect(save).toHaveBeenCalledOnce();
        });
        expect(save.mock.calls[0]?.[0]).toMatchObject({
            serverName: "tools",
            displayName: "Renamed tools",
            expectedRevision: row.revision,
        });
    },
);
