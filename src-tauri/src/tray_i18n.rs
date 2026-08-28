//! System tray menu internationalization resources.

pub struct TrayI18n {
    pub show: &'static str,
    pub pause_all: &'static str,
    pub resume_all: &'static str,
    pub speed_limit: &'static str,
    pub open_dir: &'static str,
    pub game_mode: &'static str,
    pub quit: &'static str,
}

impl TrayI18n {
    const ZH_CN: Self = Self {
        show: "显示窗口",
        pause_all: "暂停全部下载",
        resume_all: "恢复全部下载",
        speed_limit: "限速模式",
        open_dir: "打开下载目录",
        game_mode: "游戏模式",
        quit: "退出",
    };

    const EN_US: Self = Self {
        show: "Show Window",
        pause_all: "Pause All",
        resume_all: "Resume All",
        speed_limit: "Speed Limit",
        open_dir: "Open Download Dir",
        game_mode: "Game Mode",
        quit: "Quit",
    };

    pub fn for_language(lang: &str) -> &'static Self {
        if lang.starts_with("en") {
            &Self::EN_US
        } else {
            &Self::ZH_CN
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_i18n_for_language() {
        let zh = TrayI18n::for_language("zh-CN");
        assert_eq!(zh.show, "显示窗口");
        assert_eq!(zh.quit, "退出");

        let en = TrayI18n::for_language("en-US");
        assert_eq!(en.show, "Show Window");
        assert_eq!(en.quit, "Quit");

        let fallback = TrayI18n::for_language("de-DE");
        assert_eq!(fallback.show, "显示窗口");
    }
}
