export class LayoutTransliterator {
    map: Record<string, string>;
    reverseMap: Record<string, string>;
    constructor(map: Record<string, string>) {
        this.map = map;
        this.reverseMap = {};
        for (const key in map) {
            this.reverseMap[map[key]] = key;
        }
    }
    get(key: string) {
        return this.map[key];
    }
    getRev(key: string) {
        return this.reverseMap[key];
    }

    transliterate(str: string) {
        let primaryLanguageText = Array.from(str).map(c => this.get(c)).join("");
        let secondaryLanguageText = Array.from(str).map(c => this.getRev(c)).join("");
        return primaryLanguageText || secondaryLanguageText;
    }
}

export const EN_UA_TRANSLITERATOR = new LayoutTransliterator({
    "q": "й",
    "w": "ц",
    "e": "у",
    "r": "к",
    "t": "е",
    "y": "н",
    "u": "г",
    "i": "ш",
    "o": "щ",
    "p": "з",
    "[": "х",
    "]": "ї",
    "a": "ф",
    "s": "і",
    "d": "в",
    "g": "а",
    "h": "п",
    "j": "о",
    "k": "л",
    "l": "д",
    ";": "ж",
    "'": "є",
    "z": "я",
    "x": "ч",
    "c": "с",
    "v": "м",
    "b": "и",
    "n": "т",
    "m": "ь",
    ",": "б",
    ".": "ю",
    "\\": "ґ",
});
