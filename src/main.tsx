import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { LocaleStartup } from "./features/settings/LocaleStartup";
import { LanguageSelector } from "./features/settings/LanguageSelector";

const root = document.getElementById("root");
if (!root) throw new Error("The application root element is missing.");

ReactDOM.createRoot(root).render(
    <React.StrictMode>
        <LocaleStartup systemLanguage={navigator.language}>
            {(snapshot) => (
                <App languageSettings={<LanguageSelector {...snapshot} />} />
            )}
        </LocaleStartup>
    </React.StrictMode>,
);
