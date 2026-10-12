import {
    act,
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/react";
import { StrictMode, createRef, type Ref } from "react";
import type { ModuleNavigationHandle } from "../../components/moduleNavigation";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createAppI18n } from "../../i18n";
import { NotificationProvider } from "../../components/Notifications";
import {
    getSkill,
    importSkillFolder,
    listSkills,
    pickSkillFolder,
    setSkillSource,
    SkillRequestError,
    type SkillRecord,
} from "../../lib/desktop/skills";
import { SkillLibrary } from "./SkillLibrary";
vi.mock(import("../../lib/desktop/skills"), async (importOriginal) => ({
    ...(await importOriginal()),
    listSkills: vi.fn(),
    getSkill: vi.fn(),
    pickSkillFolder: vi.fn(),
    importSkillFolder: vi.fn(),
    setSkillSource: vi.fn(),
}));
const list = vi.mocked(listSkills);
const get = vi.mocked(getSkill);
const pick = vi.mocked(pickSkillFolder);
const create = vi.mocked(importSkillFolder);
const save = vi.mocked(setSkillSource);

const ID = "0123456789abcdef0123456789abcdef";
const row: SkillRecord = {
    id: ID,
    name: "demo-skill",
    description: "Explains the demo skill.",
    originPath: "/tmp/sources/demo",
    sourceUrl: null,
    sourceRevision: null,
    contentDigest: "ab".repeat(32),
    fileCount: 2,
    byteSize: 2048,
    revision: 1,
    importedAtMs: 1_700_000_000_000,
    updatedAtMs: 1_700_000_000_000,
};

function page(
    items: SkillRecord[],
    nextCursor: string | null = null,
    total = items.length,
) {
    return { kind: "desktop" as const, page: { items, nextCursor, total } };
}

async function setup(
    locale: "en" | "zh-CN" = "en",
    homeRef?: Ref<ModuleNavigationHandle>,
) {
    const i18n = await createAppI18n(locale);
    // The provider renders the same transient feedback a user sees.
    return render(
        <StrictMode>
            <I18nextProvider i18n={i18n}>
                <NotificationProvider>
                    <SkillLibrary
                        {...(homeRef === undefined ? {} : { homeRef })}
                    />
                </NotificationProvider>
            </I18nextProvider>
        </StrictMode>,
    );
}

beforeEach(() => {
    list.mockReset().mockResolvedValue(page([row]));
    get.mockReset().mockResolvedValue(row);
    pick.mockReset().mockResolvedValue({
        kind: "selected",
        path: "/tmp/sources/demo",
    });
    create.mockReset().mockResolvedValue({ imported: [row], skipped: [] });
    save.mockReset().mockResolvedValue({
        ...row,
        sourceUrl: "https://example.com/skills",
        sourceRevision: "v1.2.0",
        revision: 2,
    });
});
afterEach(() => {
    cleanup();
});

test("the list keeps one compact row per skill", async () => {
    await setup();
    expect(
        await screen.findByRole("heading", { name: "demo-skill" }),
    ).toBeDefined();
    expect(screen.getByText("Explains the demo skill.")).toBeDefined();
    // A folder import states that, instead of claiming an unknown source.
    expect(screen.getByText("Source: local import")).toBeDefined();
    // File count, size and dates belong to the detail page.
    expect(screen.queryByText("2 files")).toBeNull();
    expect(screen.queryByText("2 KB")).toBeNull();
    expect(screen.queryByText("Imported from /tmp/sources/demo")).toBeNull();
    expect(
        screen.getByRole("button", { name: "Open details for demo-skill" }),
    ).toBeDefined();
    expect(list).toHaveBeenCalledTimes(1);
});

test("the detail page shows the imported facts", async () => {
    await setup();
    fireEvent.click(
        await screen.findByRole("button", {
            name: "Open details for demo-skill",
        }),
    );
    expect(get).toHaveBeenCalledWith(ID);
    expect(
        await screen.findByRole("heading", { level: 1, name: "Skill details" }),
    ).toBeDefined();
    expect(screen.getByText("2 files")).toBeDefined();
    expect(screen.getByText("2 KB")).toBeDefined();
    expect(screen.getByText("/tmp/sources/demo")).toBeDefined();
    expect(screen.getByText("Local import")).toBeDefined();
    // The content digest stays out of the page; only facts a user can read are shown.
    expect(screen.queryByText("ab".repeat(32))).toBeNull();
    expect(screen.getByText("Imported from")).toBeDefined();
    expect(screen.getByText("Last change")).toBeDefined();
    expect(
        screen.getByText(
            "Automatic updates and deployment to an agent are not implemented yet.",
        ),
    ).toBeDefined();
});

test("a large skill reports its size in megabytes", async () => {
    // 11 MiB, the size of a real published skill; kilobytes would be unreadable.
    const large = { ...row, byteSize: 11_534_336 };
    list.mockResolvedValue(page([large]));
    get.mockResolvedValue(large);
    await setup();
    fireEvent.click(
        await screen.findByRole("button", {
            name: "Open details for demo-skill",
        }),
    );
    expect(await screen.findByText("11 MB")).toBeDefined();
});

test("the import action sits in the page header and imports a folder", async () => {
    await setup();
    const heading = await screen.findByRole("heading", {
        level: 1,
        name: "Skills",
    });
    const button = screen.getByRole("button", { name: "Local import" });
    // Same row as the title, like the other module pages.
    expect(heading.parentElement).toBe(button.parentElement);
    // Importing an existing folder is not creating a skill, so the glyph is the
    // folder icon rather than the plus used by the "New" actions.
    expect(button.querySelector("path")?.getAttribute("d")).toBe(
        "M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7l-2-2H4a2 2 0 0 0-2 2v10a2 2 0 0 0 2 2Z",
    );
});

test("the library total is stated above the list", async () => {
    // 7 skills, one page loaded: the count describes the library, not the page.
    list.mockResolvedValue(page([row], null, 7));
    await setup();
    expect(await screen.findByText("7 skills in total")).toBeDefined();
});

test("an empty library explains what to do next without a count", async () => {
    list.mockResolvedValue(page([], null, 0));
    await setup();
    expect(
        await screen.findByText("No skill folder has been imported yet."),
    ).toBeDefined();
    expect(screen.queryByText(/in total/)).toBeNull();
});

test("local import asks for a folder and imports everything in it", async () => {
    const second: SkillRecord = { ...row, id: "f".repeat(32), name: "zebra" };
    create.mockResolvedValue({ imported: [row, second], skipped: [] });
    // The re-read list is what the page shows, so it holds both skills.
    list.mockResolvedValueOnce(page([row]));
    list.mockResolvedValueOnce(page([row, second]));
    await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Local import" }),
    );
    await waitFor(() => {
        expect(pick).toHaveBeenCalledWith("Choose a skill folder");
    });
    expect(create).toHaveBeenCalledWith("/tmp/sources/demo");
    // Both rows are announced and listed, and the first page is re-read.
    expect(await screen.findByText("Imported 2 skills.")).toBeDefined();
    await waitFor(() => {
        expect(list).toHaveBeenCalledTimes(2);
    });
    expect(screen.getByRole("heading", { name: "zebra" })).toBeDefined();
});

test("closing the picker changes nothing", async () => {
    pick.mockResolvedValue({ kind: "cancelled" });
    await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Local import" }),
    );
    await waitFor(() => {
        expect(pick).toHaveBeenCalledTimes(1);
    });
    expect(create).not.toHaveBeenCalled();
    expect(list).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Nothing was imported.")).toBeNull();
});

test("refused folders are named with their reason", async () => {
    create.mockResolvedValue({
        imported: [row],
        skipped: [
            { folder: "broken", code: "manifest_invalid" },
            { folder: "duplicate", code: "name_taken" },
        ],
    });
    await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Local import" }),
    );
    expect(await screen.findByText("Imported 1 skill.")).toBeDefined();
    expect(screen.getByText("2 folders were skipped:")).toBeDefined();
    expect(
        screen.getByText(
            "broken: SKILL.md needs YAML frontmatter with a name and a one-line description.",
        ),
    ).toBeDefined();
    expect(
        screen.getByText(
            "duplicate: A skill with this name is already in the library.",
        ),
    ).toBeDefined();
});

test("a folder with no skills is explained and reported", async () => {
    create.mockResolvedValue({
        imported: [],
        skipped: [{ folder: "empty", code: "manifest_missing" }],
    });
    await setup();
    fireEvent.click(
        await screen.findByRole("button", { name: "Local import" }),
    );
    expect(await screen.findByText("Nothing was imported.")).toBeDefined();
    expect(
        screen.getByText("empty: The folder has no SKILL.md file."),
    ).toBeDefined();
    // No success claim is shown for an import that added nothing.
    expect(screen.queryByText("Imported 0 skills.")).toBeNull();
});

test("import failures are translated and unknown outcomes require a reload", async () => {
    await setup();
    pick.mockRejectedValue(new SkillRequestError("no_skills_found"));
    fireEvent.click(
        await screen.findByRole("button", { name: "Local import" }),
    );
    expect(
        await screen.findByText(
            "That folder holds no skill: no SKILL.md was found in it or in its subfolders.",
        ),
    ).toBeDefined();
    expect(screen.queryByRole("button", { name: "Reload list" })).toBeNull();

    // The second attempt reaches the import step again.
    pick.mockResolvedValue({ kind: "selected", path: "/tmp/sources/demo" });
    create.mockRejectedValue(new SkillRequestError("outcome_unknown"));
    fireEvent.click(screen.getByRole("button", { name: "Local import" }));
    expect(
        await screen.findByText(
            "The result is unknown. Reload the list before another change.",
        ),
    ).toBeDefined();
    const reload = screen.getByRole("button", { name: "Reload list" });
    list.mockResolvedValue(page([row]));
    fireEvent.click(reload);
    await waitFor(() => {
        expect(list).toHaveBeenCalledTimes(2);
    });
});

test("loading more pages keeps the name order and skips repeats", async () => {
    const second: SkillRecord = { ...row, id: "f".repeat(32), name: "zebra" };
    const cursor = `${row.name}\n${row.id}`;
    list.mockResolvedValueOnce(page([row], cursor));
    list.mockResolvedValueOnce(page([row, second], null));
    await setup();
    fireEvent.click(await screen.findByRole("button", { name: "Load more" }));
    expect(await screen.findByRole("heading", { name: "zebra" })).toBeDefined();
    expect(list).toHaveBeenLastCalledWith(cursor);
    expect(screen.getAllByRole("heading", { name: "demo-skill" })).toHaveLength(
        1,
    );
    // The last page no longer offers more rows.
    expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
});

test("the detail page records an address without leaving the skill", async () => {
    await setup();
    fireEvent.click(
        await screen.findByRole("button", {
            name: "Open details for demo-skill",
        }),
    );
    expect(get).toHaveBeenCalledWith(ID);
    const url = await screen.findByLabelText("Source address");
    const revision = screen.getByLabelText("Revision");
    // A revision without an address is refused before any request.
    fireEvent.change(revision, { target: { value: "v9" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(
        screen.getByText(
            "A revision can only be saved together with a source address.",
        ),
    ).toBeDefined();
    expect(save).not.toHaveBeenCalled();

    fireEvent.change(url, {
        target: { value: "https://example.com/skills" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => {
        expect(save).toHaveBeenCalledWith({
            id: ID,
            expectedRevision: 1,
            url: "https://example.com/skills",
            revision: "v9",
        });
    });
    // The page keeps showing the skill, now with the stored source (the mock
    // returns v1.2.0) instead of the unknown-source line.
    expect(
        await screen.findByText("https://example.com/skills · revision v1.2.0"),
    ).toBeDefined();
    expect(screen.getByText("Source saved.")).toBeDefined();
    expect(screen.getByLabelText("Source address")).toBeDefined();
    expect(screen.queryByText("Source: local import")).toBeNull();
    // The list row was updated in place, so going back needs no second read.
    fireEvent.click(screen.getByRole("button", { name: "Back to skills" }));
    expect(
        await screen.findByText(
            "Source: https://example.com/skills · revision v1.2.0",
        ),
    ).toBeDefined();
    expect(list).toHaveBeenCalledTimes(1);
});

test("a stale source revision asks for a reload instead of overwriting", async () => {
    await setup();
    fireEvent.click(
        await screen.findByRole("button", {
            name: "Open details for demo-skill",
        }),
    );
    const url = await screen.findByLabelText("Source address");
    save.mockRejectedValue(new SkillRequestError("revision_conflict"));
    fireEvent.change(url, {
        target: { value: "https://example.com/skills" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(
        await screen.findByText(
            "Another change was saved first. Reload and try again.",
        ),
    ).toBeDefined();
    const reload = screen.getByRole("button", { name: "Reload list" });
    // The re-read supplies the newer revision, so a retry cannot overwrite it.
    get.mockResolvedValue({ ...row, revision: 3 });
    fireEvent.click(reload);
    await waitFor(() => {
        expect(get).toHaveBeenCalledTimes(2);
    });
    // The reopened form starts from the stored values again.
    fireEvent.change(await screen.findByLabelText("Source address"), {
        target: { value: "https://example.com/skills" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => {
        expect(save).toHaveBeenLastCalledWith({
            id: ID,
            expectedRevision: 3,
            url: "https://example.com/skills",
            revision: null,
        });
    });
});

test("the browser preview reads nothing", async () => {
    list.mockResolvedValue({ kind: "preview" });
    await setup();
    expect(
        await screen.findByText(
            "Importing and listing skills needs the desktop app. This browser preview reads nothing.",
        ),
    ).toBeDefined();
    expect(screen.queryByRole("heading", { name: "demo-skill" })).toBeNull();
});

test("the Chinese page shows the same states and controls", async () => {
    await setup("zh-CN");
    expect(
        await screen.findByRole("heading", { level: 1, name: "技能" }),
    ).toBeDefined();
    expect(screen.getByRole("button", { name: "本地导入" })).toBeDefined();
    // The list arrives asynchronously; wait for the first row before reading it.
    expect(
        await screen.findByRole("heading", { name: "demo-skill" }),
    ).toBeDefined();
    expect(screen.getByText("来源：本地导入")).toBeDefined();
    fireEvent.click(
        screen.getByRole("button", { name: "查看 demo-skill 的详情" }),
    );
    expect(
        await screen.findByRole("heading", { level: 1, name: "技能详情" }),
    ).toBeDefined();
    expect(screen.getByText("2 个文件")).toBeDefined();
    expect(screen.getByText("导入自")).toBeDefined();
});

test("the sidebar home returns from the detail page", async () => {
    const homeRef = createRef<ModuleNavigationHandle>();
    await setup("en", homeRef);
    fireEvent.click(
        await screen.findByRole("button", {
            name: "Open details for demo-skill",
        }),
    );
    await screen.findByLabelText("Source address");
    act(() => {
        expect(homeRef.current?.openHome()).toBe(true);
    });
    expect(screen.queryByLabelText("Source address")).toBeNull();
    expect(screen.getByRole("heading", { name: "Skills" })).toBeDefined();
});
