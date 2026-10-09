/// <reference types="vite/client" />

interface ImportMetaEnv {
    /** Target OS for this Tauri build (`darwin`, `windows`, or `linux`). */
    readonly TAURI_ENV_PLATFORM?: string;
}

interface ImportMeta {
    readonly env: ImportMetaEnv;
}
