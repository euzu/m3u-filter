export interface UiConfig {
    languages?: string[],
    app_title?: string,
    app_logo?: string,
    api?: {
        apiUrl?: string
        authUrl?: string
    }
}

export const DefaultUiConfig: UiConfig = {
    languages: ["en", "de", "fr", "tr", "es", "it", "ru"],
    app_title: "tuliprox",
    api: {
        apiUrl: "/api/v1/",
        authUrl: "/auth/"
    }
}