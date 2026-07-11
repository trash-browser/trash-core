import json
from pathlib import Path

pages = {
    "you": ["sync", "services"],
    "autofill": ["password-manager", "payment", "addresses"],
    "privacy": ["clear", "cookies", "safe-browsing", "site-settings"],
    "appearance": ["theme", "toolbar", "bookmarks-bar", "font-size", "zoom"],
    "search": ["engine", "manage"],
    "default": ["make-default"],
    "startup": ["open", "continue", "specific"],
    "languages": ["preferred", "spellcheck", "translate"],
    "downloads": ["location", "ask"],
    "a11y": ["captions", "highlight", "tab-hover"],
    "system": ["hw", "bg", "proxy"],
    "reset": ["restore"],
    "about": ["version", "help"],
}

bodies_en = {
    "you": {
        "sync": "Sync bookmarks, passwords, and settings when you sign in to Trash.",
        "services": "Manage Trash account services connected to this browser profile.",
    },
    "autofill": {
        "password-manager": "View and manage saved passwords and passkeys.",
        "payment": "Save cards and fill payment forms automatically.",
        "addresses": "Save addresses for faster checkout and forms.",
    },
    "privacy": {
        "clear": "Choose a time range and data types to delete from this device.",
        "cookies": "Control whether sites can use third-party cookies.",
        "safe-browsing": "Choose how strongly Trash protects you from unsafe sites.",
        "site-settings": "Permissions for camera, mic, location, notifications, and more.",
    },
    "appearance": {
        "theme": "Choose light, dark, or follow your device theme.",
        "toolbar": "Show or hide the Home button on the toolbar.",
        "bookmarks-bar": "Show the bookmarks bar below the address bar.",
        "font-size": "Adjust the default font size for web pages.",
        "zoom": "Set the default zoom level for web pages.",
    },
    "search": {
        "engine": "Pick the search engine used from the address bar.",
        "manage": "Add, edit, or remove search engines and site search shortcuts.",
    },
    "default": {
        "make-default": "Make Trash Browser open links from other apps by default.",
    },
    "startup": {
        "open": "Open a fresh New Tab page every time Trash starts.",
        "continue": "Restore windows and tabs from your last session.",
        "specific": "Choose one or more pages to open on startup.",
    },
    "languages": {
        "preferred": "Choose the language used for Trash Browser settings and UI.",
        "spellcheck": "Check spelling as you type in web forms.",
        "translate": "Offer to translate pages written in other languages.",
    },
    "downloads": {
        "location": "Choose the folder where downloads are saved.",
        "ask": "Ask for a save location before each download starts.",
    },
    "a11y": {
        "captions": "Customize caption preferences for media.",
        "highlight": "Draw a highlight around the focused control.",
        "tab-hover": "Show a preview card when hovering a tab.",
    },
    "system": {
        "hw": "Use the GPU to improve page and video performance.",
        "bg": "Allow Trash processes to keep running after the window closes.",
        "proxy": "Open your operating system proxy configuration.",
    },
    "reset": {
        "restore": "Reset selected settings back to Trash Browser defaults.",
    },
    "about": {
        "version": "Version details and update status for this installation.",
        "help": "Get help docs, tips, and support resources.",
    },
}


def make_sub(bodies, option_titles):
    sub = {
        "common": {
            "back": option_titles["back"],
            "body": option_titles["body"],
            "option": option_titles["option"],
        }
    }
    for page, rows in pages.items():
        sub[page] = {}
        for row in rows:
            entry = {
                "body": bodies[page][row],
                "options": {
                    "a": {
                        "title": option_titles["a"],
                        "description": option_titles["a_desc"],
                    },
                    "b": {
                        "title": option_titles["b"],
                        "description": option_titles["b_desc"],
                    },
                    "c": {
                        "title": option_titles["c"],
                        "description": option_titles["c_desc"],
                    },
                    "toggle": {
                        "title": option_titles["toggle"],
                        "description": option_titles["toggle_desc"],
                    },
                },
            }
            sub[page][row] = entry

    sub["appearance"]["theme"]["options"] = {
        "device": {
            "title": option_titles["theme_device"],
            "description": option_titles["theme_device_d"],
        },
        "light": {
            "title": option_titles["theme_light"],
            "description": option_titles["theme_light_d"],
        },
        "dark": {
            "title": option_titles["theme_dark"],
            "description": option_titles["theme_dark_d"],
        },
    }
    sub["appearance"]["font-size"]["options"] = {
        "small": {"title": option_titles["fs_s"]},
        "medium": {"title": option_titles["fs_m"]},
        "large": {"title": option_titles["fs_l"]},
        "xlarge": {"title": option_titles["fs_xl"]},
    }
    sub["privacy"]["safe-browsing"]["options"] = {
        "enhanced": {
            "title": option_titles["sb_e"],
            "description": option_titles["sb_e_d"],
        },
        "standard": {
            "title": option_titles["sb_s"],
            "description": option_titles["sb_s_d"],
        },
        "off": {
            "title": option_titles["sb_o"],
            "description": option_titles["sb_o_d"],
        },
    }
    sub["privacy"]["clear"]["options"] = {
        "lastHour": {"title": option_titles["clear_h"]},
        "lastDay": {"title": option_titles["clear_d"]},
        "allTime": {"title": option_titles["clear_a"]},
    }
    sub["search"]["engine"]["options"] = {
        "bing": {"title": "Bing"},
        "google": {"title": "Google"},
        "duckduckgo": {"title": "DuckDuckGo"},
    }
    sub["languages"]["preferred"]["options"] = {}
    return sub


en_opts = {
    "back": "Back",
    "body": "Configure “{{title}}” for Trash Browser.",
    "option": "Option {{id}}",
    "a": "Primary option",
    "a_desc": "Recommended for most people",
    "b": "Additional control",
    "b_desc": "Fine-tune this feature",
    "c": "Advanced",
    "c_desc": "More detailed preferences",
    "toggle": "Enable this setting",
    "toggle_desc": "Turn this feature on or off",
    "theme_device": "Device",
    "theme_device_d": "Match your system appearance",
    "theme_light": "Light",
    "theme_light_d": "Light backgrounds and dark text",
    "theme_dark": "Dark",
    "theme_dark_d": "Dark backgrounds and light text",
    "fs_s": "Small",
    "fs_m": "Medium (Recommended)",
    "fs_l": "Large",
    "fs_xl": "Very large",
    "sb_e": "Enhanced protection",
    "sb_e_d": "Proactive protection against dangerous sites",
    "sb_s": "Standard protection",
    "sb_s_d": "Protects against known dangerous sites",
    "sb_o": "No protection (not recommended)",
    "sb_o_d": "You will not get Safe Browsing warnings",
    "clear_h": "Last hour",
    "clear_d": "Last 24 hours",
    "clear_a": "All time",
}

th_opts = {
    "back": "กลับ",
    "body": "ตั้งค่า “{{title}}” สำหรับ Trash Browser",
    "option": "ตัวเลือก {{id}}",
    "a": "ตัวเลือกหลัก",
    "a_desc": "แนะนำสำหรับผู้ใช้ทั่วไป",
    "b": "การควบคุมเพิ่มเติม",
    "b_desc": "ปรับแต่งฟีเจอร์นี้",
    "c": "ขั้นสูง",
    "c_desc": "การตั้งค่าโดยละเอียด",
    "toggle": "เปิดใช้การตั้งค่านี้",
    "toggle_desc": "เปิดหรือปิดฟีเจอร์นี้",
    "theme_device": "อุปกรณ์",
    "theme_device_d": "ตามธีมของระบบ",
    "theme_light": "สว่าง",
    "theme_light_d": "พื้นหลังสว่าง ข้อความเข้ม",
    "theme_dark": "มืด",
    "theme_dark_d": "พื้นหลังมืด ข้อความสว่าง",
    "fs_s": "เล็ก",
    "fs_m": "ปานกลาง (แนะนำ)",
    "fs_l": "ใหญ่",
    "fs_xl": "ใหญ่มาก",
    "sb_e": "การป้องกันขั้นสูง",
    "sb_e_d": "ป้องกันเชิงรุกจากเว็บไซต์อันตราย",
    "sb_s": "การป้องกันมาตรฐาน",
    "sb_s_d": "ป้องกันเว็บไซต์อันตรายที่รู้จัก",
    "sb_o": "ไม่มีการป้องกัน (ไม่แนะนำ)",
    "sb_o_d": "คุณจะไม่ได้รับการเตือน Safe Browsing",
    "clear_h": "1 ชั่วโมงที่แล้ว",
    "clear_d": "24 ชั่วโมงที่แล้ว",
    "clear_a": "ทั้งหมด",
}

zh_opts = {
    "back": "返回",
    "body": "为 Trash Browser 配置“{{title}}”。",
    "option": "选项 {{id}}",
    "a": "主要选项",
    "a_desc": "适合大多数用户",
    "b": "其他控件",
    "b_desc": "微调此功能",
    "c": "高级",
    "c_desc": "更详细的偏好设置",
    "toggle": "启用此设置",
    "toggle_desc": "打开或关闭此功能",
    "theme_device": "设备",
    "theme_device_d": "跟随系统外观",
    "theme_light": "浅色",
    "theme_light_d": "浅色背景与深色文字",
    "theme_dark": "深色",
    "theme_dark_d": "深色背景与浅色文字",
    "fs_s": "小",
    "fs_m": "中等（推荐）",
    "fs_l": "大",
    "fs_xl": "特大",
    "sb_e": "增强保护",
    "sb_e_d": "主动防范危险网站",
    "sb_s": "标准保护",
    "sb_s_d": "防范已知危险网站",
    "sb_o": "无保护（不推荐）",
    "sb_o_d": "您将不会收到安全浏览警告",
    "clear_h": "过去 1 小时",
    "clear_d": "过去 24 小时",
    "clear_a": "全部时间",
}

ja_opts = {
    "back": "戻る",
    "body": "Trash Browser の「{{title}}」を設定します。",
    "option": "オプション {{id}}",
    "a": "主なオプション",
    "a_desc": "ほとんどのユーザーに推奨",
    "b": "追加のコントロール",
    "b_desc": "この機能を細かく調整",
    "c": "詳細設定",
    "c_desc": "より詳しい設定",
    "toggle": "この設定を有効にする",
    "toggle_desc": "この機能のオン / オフ",
    "theme_device": "デバイス",
    "theme_device_d": "システムの外観に合わせる",
    "theme_light": "ライト",
    "theme_light_d": "明るい背景と暗い文字",
    "theme_dark": "ダーク",
    "theme_dark_d": "暗い背景と明るい文字",
    "fs_s": "小",
    "fs_m": "中（推奨）",
    "fs_l": "大",
    "fs_xl": "特大",
    "sb_e": "強化された保護",
    "sb_e_d": "危険なサイトからプロアクティブに保護",
    "sb_s": "標準保護",
    "sb_s_d": "既知の危険なサイトから保護",
    "sb_o": "保護なし（非推奨）",
    "sb_o_d": "セーフ ブラウジングの警告は表示されません",
    "clear_h": "過去 1 時間",
    "clear_d": "過去 24 時間",
    "clear_a": "全期間",
}


def fill_bodies(template: str):
    return {p: {r: template for r in rows} for p, rows in bodies_en.items()}


bodies_th = fill_bodies("ปรับแต่งการตั้งค่านี้ใน Trash Browser")
bodies_zh = fill_bodies("在 Trash Browser 中配置此设置。")
bodies_ja = fill_bodies("Trash Browser でこの設定を構成します。")
bodies_th["languages"]["preferred"] = "เลือกภาษาสำหรับ Trash Browser"
bodies_zh["languages"]["preferred"] = "选择 Trash Browser 使用的语言"
bodies_ja["languages"]["preferred"] = "Trash Browser で使用する言語を選択します"
bodies_th["appearance"]["theme"] = "เลือกโหมดสว่าง มืด หรือตามอุปกรณ์"
bodies_zh["appearance"]["theme"] = "选择浅色、深色或跟随设备"
bodies_ja["appearance"]["theme"] = "ライト、ダーク、またはデバイスに合わせる"

hints = {
    "en": "Choose the language for Trash Browser settings and UI.",
    "th": "เลือกภาษาสำหรับหน้าตั้งค่าและส่วนติดต่อผู้ใช้ของ Trash Browser",
    "zh": "选择 Trash Browser 设置和界面所用的语言。",
    "ja": "Trash Browser の設定と UI で使用する言語を選択します。",
}

root = Path(__file__).resolve().parents[1] / "src" / "i18n" / "locales"


def merge(locale: str, sub: dict):
    path = root / f"{locale}.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    data["sub"] = sub
    data["pages"]["languages"]["preferred"]["pickerHint"] = hints[locale]
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print("updated", locale)


merge("en", make_sub(bodies_en, en_opts))
merge("th", make_sub(bodies_th, th_opts))
merge("zh", make_sub(bodies_zh, zh_opts))
merge("ja", make_sub(bodies_ja, ja_opts))
print("done")
