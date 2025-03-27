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
    languages: ["en", "de", "fr", "tr", "es", "it", "ru", "zh-CN", "zh-HK"],
    app_title: "m3u-filter",
    api: {
        apiUrl: "/api/v1/",
        authUrl: "/auth/"
    }
}