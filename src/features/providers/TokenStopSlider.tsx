import type { CSSProperties, JSX } from "react";
import { useTranslation } from "react-i18next";
import { TOKEN_STOPS } from "./tokenStops";
import styles from "./TokenStopSlider.module.css";

/** Inputs for {@link TokenStopSlider}. */
export interface TokenStopSliderProps {
    id: string;
    /** A preset token count, or `null` when the limit is unset. */
    value: number | null;
    disabled: boolean;
    onChange: (tokens: number | null) => void;
}

/**
 * Discrete slider shared by context window and maximum output. Position 0 is
 * unset; each later position is one {@link TOKEN_STOPS} entry.
 */
export function TokenStopSlider({
    id,
    value,
    disabled,
    onChange,
}: TokenStopSliderProps): JSX.Element {
    const { t } = useTranslation();
    const index = TOKEN_STOPS.findIndex((stop) => stop.tokens === value);
    const position = index < 0 ? 0 : index + 1;
    const currentLabel = TOKEN_STOPS[position - 1]?.label;
    const unsetLabel = t("providers.models.parameters.unset");
    const current = currentLabel === undefined ? unsetLabel : currentLabel;

    function choose(next: number): void {
        if (disabled) return;
        if (next <= 0) {
            onChange(null);
            return;
        }
        const stop = TOKEN_STOPS[next - 1];
        if (stop !== undefined) onChange(stop.tokens);
    }

    // The filled length follows the selected stop. Track colors are theme tokens,
    // so a native accent (which paints the whole control in the text color)
    // cannot express the empty and filled parts separately.
    const sliderStyle = {
        "--slider-fill": `${String((position / TOKEN_STOPS.length) * 100)}%`,
        "--tick-columns": String(TOKEN_STOPS.length + 1),
    } as CSSProperties;
    return (
        <div className={styles["slider"]} style={sliderStyle}>
            <input
                id={id}
                className={styles["range"]}
                type="range"
                min={0}
                max={TOKEN_STOPS.length}
                step={1}
                value={position}
                disabled={disabled}
                aria-valuetext={current}
                onChange={(event) => {
                    choose(Number(event.currentTarget.value));
                }}
            />
            <div className={styles["ticks"]}>
                <span className={styles["tick"]}>
                    {position === 0 ? unsetLabel : ""}
                </span>
                {TOKEN_STOPS.map((stop, tick) => (
                    <button
                        key={stop.label}
                        type="button"
                        className={
                            position === tick + 1
                                ? [styles["tick"], styles["tickCurrent"]].join(
                                      " ",
                                  )
                                : styles["tick"]
                        }
                        disabled={disabled}
                        onClick={() => {
                            choose(tick + 1);
                        }}
                    >
                        {stop.label}
                    </button>
                ))}
            </div>
        </div>
    );
}
