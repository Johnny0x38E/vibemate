/** Locale-aware timestamp shared by the Agent history cards. */
export function formatTime(ms: number, locale: string): string {
    return new Intl.DateTimeFormat(locale, {
        dateStyle: "medium",
        timeStyle: "short",
    }).format(new Date(ms));
}
