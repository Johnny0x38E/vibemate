/**
 * Sidebar navigation without remounting a feature module.
 * Each module owns its home state and busy/dirty guards; App only requests the
 * transition and switches destinations when the module allows it.
 */
export interface ModuleNavigationHandle {
    /** Return to the module home; false means navigation was blocked/cancelled. */
    openHome: () => boolean;
}
